# ZK Insurance Verifier

A zero-knowledge proof system for verifying insurance discount eligibility based on age and BMI without revealing the actual values.

## Features

- Proves age is between 10-25 years
- Proves BMI is between 18.5-24.9
- Generates zero-knowledge proofs without revealing actual values
- Docker containerized for easy deployment
- TCP server interface accessible via `nc` or `telnet`
- Concurrent client support

## Prerequisites

- Docker and Docker Compose
- Git

## Run Locally

1. Compile the circuit:
```bash
cd noir-circuit
nargo compile
```

2. Run the server:
```bash
cd server
cargo run
```

3. In a new terminal, connect to the server:
```bash
nc 127.0.0.1 8080
```

4. Follow the prompts to enter your age (10-25) and BMI multiplied by 10 (185-249).

## Usage Example

1. Build Docker Image and Deploy on Docker Hub:
```bash
docker build -t ayushranjan123/insurance-verifier:latest --push .
```

2. Start the server:
```bash
docker run --rm --init -p 8080:8080 ayushranjan123/insurance-verifier
```

3. Connect from another terminal:
```bash
nc 127.0.0.1 8080
```

4. Deploy On Oyster TEE:
```bash
oyster-cvm deploy --wallet-private-key <key> --duration-in-minutes 15 --docker-compose docker-compose.yml --arch amd64
```

5. Interaction example:
```
ZK Insurance Verifier Server
============================
Enter age (10-25): 20
Enter BMI multiplied by 10 (185-249): 220
Generating proof...

=== PROOF RESPONSE ===
Success: true
Message: Proof generated successfully! The user is eligible for insurance discount.
...
```