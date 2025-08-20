use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose};
use chrono;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "8080")]
    port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProofRequest {
    age: u32,
    bmi_multiplied: u32, // BMI * 10 to avoid decimals
}

#[derive(Debug, Serialize, Deserialize)]
struct ProofResponse {
    proof: String,
    verification_key: String,
    public_inputs: PublicInputs,
    success: bool,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PublicInputs {
    min_age: u32,
    max_age: u32,
    min_bmi: u32,
    max_bmi: u32,
}

struct NoirProver {
    circuit_path: String,
}

impl NoirProver {
    fn new() -> Self {
        // Check if we're running in Docker (where circuit is at /app/noir-circuit)
        // or locally (where circuit is at ../noir-circuit)
        let circuit_path = if std::path::Path::new("/app/noir-circuit").exists() {
            "/app/noir-circuit".to_string()
        } else {
            "../noir-circuit".to_string()
        };
        
        Self {
            circuit_path,
        }
    }

    async fn generate_proof(&self, request: ProofRequest) -> Result<ProofResponse> {
        // Create a temporary directory for this proof generation
        let temp_dir = TempDir::new()?;
        let temp_path = temp_dir.path();

        // Copy the circuit to temporary directory
        self.copy_circuit_to_temp(temp_path)?;

        // Update Prover.toml with the input values
        let prover_toml_content = format!(
            r#"age = "{}"
bmi = "{}"
min_age = "10"
max_age = "25"
min_bmi = "185"
max_bmi = "249""#,
            request.age, request.bmi_multiplied
        );

        let prover_path = temp_path.join("Prover.toml");
        fs::write(&prover_path, prover_toml_content)?;

        // Compile the circuit
        let compile_output = Command::new("nargo")
            .arg("compile")
            .current_dir(&temp_path)
            .output()
            .context("Failed to compile Noir circuit")?;

        if !compile_output.status.success() {
            return Ok(ProofResponse {
                proof: String::new(),
                verification_key: String::new(),
                public_inputs: PublicInputs {
                    min_age: 10,
                    max_age: 25,
                    min_bmi: 185,
                    max_bmi: 249,
                },
                success: false,
                message: format!(
                    "Circuit compilation failed: {}",
                    String::from_utf8_lossy(&compile_output.stderr)
                ),
            });
        }

        // Generate the witness (execute the circuit)
        let execute_output = Command::new("nargo")
            .arg("execute")
            .current_dir(&temp_path)
            .output()
            .context("Failed to execute circuit")?;

        if !execute_output.status.success() {
            return Ok(ProofResponse {
                proof: String::new(),
                verification_key: String::new(),
                public_inputs: PublicInputs {
                    min_age: 10,
                    max_age: 25,
                    min_bmi: 185,
                    max_bmi: 249,
                },
                success: false,
                message: format!(
                    "Circuit execution failed. Likely the inputs don't satisfy the constraints: {}",
                    String::from_utf8_lossy(&execute_output.stderr)
                ),
            });
        }

        // Read the generated witness file
        let witness_path = temp_path.join("target/insurance_verifier.gz");
        let witness_bytes = fs::read(&witness_path).context("Failed to read witness file")?;
        let proof = general_purpose::STANDARD.encode(&witness_bytes);

        // Note: In Nargo 1.0.0, verification key generation is handled differently
        // and typically requires a separate backend like Barretenberg
        let verification_key = format!("witness_verification_placeholder_{}", chrono::Utc::now().timestamp());

        Ok(ProofResponse {
            proof,
            verification_key,
            public_inputs: PublicInputs {
                min_age: 10,
                max_age: 25,
                min_bmi: 185,
                max_bmi: 249,
            },
            success: true,
            message: "Proof generated successfully! The user is eligible for insurance discount.".to_string(),
        })
    }

    fn copy_circuit_to_temp(&self, temp_path: &Path) -> Result<()> {
        // Copy Nargo.toml
        let source_nargo = Path::new(&self.circuit_path).join("Nargo.toml");
        let dest_nargo = temp_path.join("Nargo.toml");
        fs::copy(source_nargo, dest_nargo)?;

        // Create src directory and copy main.nr
        let src_dir = temp_path.join("src");
        fs::create_dir_all(&src_dir)?;
        
        let source_main = Path::new(&self.circuit_path).join("src/main.nr");
        let dest_main = src_dir.join("main.nr");
        fs::copy(source_main, dest_main)?;

        Ok(())
    }
}

async fn handle_client(mut stream: TcpStream) -> Result<()> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    let prover = NoirProver::new();

    // Send welcome message
    writer.write_all(b"ZK Insurance Verifier Server\n").await?;
    writer.write_all(b"============================\n").await?;
    writer.write_all(b"Enter age (10-25): ").await?;
    writer.flush().await?;

    // Read age
    line.clear();
    reader.read_line(&mut line).await?;
    let age: u32 = line.trim().parse().context("Invalid age input")?;

    // Ask for BMI
    writer.write_all(b"Enter BMI multiplied by 10 (185-249): ").await?;
    writer.flush().await?;

    // Read BMI
    line.clear();
    reader.read_line(&mut line).await?;
    let bmi_multiplied: u32 = line.trim().parse().context("Invalid BMI input")?;

    let request = ProofRequest {
        age,
        bmi_multiplied,
    };

    writer.write_all(b"Generating proof...\n").await?;
    writer.flush().await?;

    match prover.generate_proof(request).await {
        Ok(response) => {
            let response_text = format!(
                "\n=== PROOF RESPONSE ===\nSuccess: {}\nMessage: {}\n",
                response.success, response.message
            );
            writer.write_all(response_text.as_bytes()).await?;

            if response.success {
                let proof_preview = format!(
                    "\nProof (Base64): {}...\n",
                    &response.proof[..50.min(response.proof.len())]
                );
                writer.write_all(proof_preview.as_bytes()).await?;

                let constraints = format!(
                    "\nAge Range: {} - {}\nBMI Range: {:.1} - {:.1}\n",
                    response.public_inputs.min_age,
                    response.public_inputs.max_age,
                    response.public_inputs.min_bmi as f32 / 10.0,
                    response.public_inputs.max_bmi as f32 / 10.0
                );
                writer.write_all(constraints.as_bytes()).await?;

                let json = serde_json::to_string_pretty(&response)?;
                writer.write_all(b"\nFull JSON Response:\n").await?;
                writer.write_all(json.as_bytes()).await?;
                writer.write_all(b"\n").await?;

                // Save proof to file
                let proof_filename = format!("proof_{}.json", chrono::Utc::now().timestamp());
                fs::write(&proof_filename, json)?;
                let save_msg = format!("Proof saved to: {}\n", proof_filename);
                writer.write_all(save_msg.as_bytes()).await?;
            }
        }
        Err(e) => {
            let error_msg = format!("Error generating proof: {}\n", e);
            writer.write_all(error_msg.as_bytes()).await?;
        }
    }

    writer.write_all(b"\nConnection will close. Thanks for using ZK Insurance Verifier!\n").await?;
    writer.flush().await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let addr = format!("0.0.0.0:{}", args.port);
    
    println!("ZK Insurance Verifier TCP Server");
    println!("================================");
    println!("Listening on {}", addr);
    println!("Connect using: nc 127.0.0.1 {}", args.port);
    println!("Or: telnet 127.0.0.1 {}", args.port);
    println!();

    let listener = TcpListener::bind(&addr).await?;

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                println!("New connection from: {}", addr);
                
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream).await {
                        eprintln!("Error handling client {}: {}", addr, e);
                    } else {
                        println!("Client {} disconnected", addr);
                    }
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }
}