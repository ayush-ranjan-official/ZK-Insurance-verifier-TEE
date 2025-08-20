FROM rust:1.89 as builder

# Install Noir
RUN curl -L https://raw.githubusercontent.com/noir-lang/noirup/refs/heads/main/install | bash
ENV PATH="/root/.nargo/bin:${PATH}"
RUN noirup

# Copy circuit files
WORKDIR /app
COPY noir-circuit ./noir-circuit

# Build the circuit
WORKDIR /app/noir-circuit
RUN nargo compile

# Copy and build server
WORKDIR /app
COPY server ./server
WORKDIR /app/server
RUN cargo build --release

# Runtime stage
FROM ubuntu:24.04

RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    curl \
    git \
    && rm -rf /var/lib/apt/lists/*

# Install Noir in runtime
RUN curl -L https://raw.githubusercontent.com/noir-lang/noirup/refs/heads/main/install | bash
ENV PATH="/root/.nargo/bin:${PATH}"
RUN noirup

WORKDIR /app

# Copy circuit and compiled server
COPY --from=builder /app/noir-circuit ./noir-circuit
COPY --from=builder /app/server/target/release/zk-insurance-server ./

EXPOSE 8080

CMD ["./zk-insurance-server"]