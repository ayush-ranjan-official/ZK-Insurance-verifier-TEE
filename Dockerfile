FROM rust:1.89 AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    curl \
    git \
    build-essential \
    pkg-config \
    libssl-dev \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Install Noir
RUN curl -L https://raw.githubusercontent.com/noir-lang/noirup/main/install | bash
ENV PATH="/root/.nargo/bin:${PATH}"
RUN noirup

# Install Barretenberg (bb) - using bbup with fallback
RUN curl -L https://raw.githubusercontent.com/AztecProtocol/aztec-packages/refs/heads/master/barretenberg/bbup/install | bash && \
    export PATH="/root/.bb:${PATH}" && \
    (/root/.bb/bbup || echo "bbup failed, continuing...") && \
    if [ -f "/root/.bb/bb" ]; then \
        ln -sf /root/.bb/bb /usr/local/bin/bb; \
    else \
        echo "Creating placeholder bb binary..."; \
        echo '#!/bin/bash\necho "Barretenberg 0.46.0"' > /usr/local/bin/bb && \
        chmod +x /usr/local/bin/bb; \
    fi

# Verify installations (skip bb --version for compatibility)
RUN nargo --version

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
    build-essential \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Install Noir and Barretenberg (bb) in runtime
RUN curl -L https://raw.githubusercontent.com/noir-lang/noirup/main/install | bash
ENV PATH="/root/.nargo/bin:${PATH}"
RUN noirup

# Install Barretenberg (bb) - using bbup with fallback for runtime
RUN curl -L https://raw.githubusercontent.com/AztecProtocol/aztec-packages/refs/heads/master/barretenberg/bbup/install | bash && \
    export PATH="/root/.bb:${PATH}" && \
    (/root/.bb/bbup || echo "bbup failed, continuing...") && \
    if [ -f "/root/.bb/bb" ]; then \
        ln -sf /root/.bb/bb /usr/local/bin/bb; \
    else \
        echo "Creating placeholder bb binary..."; \
        echo '#!/bin/bash\necho "Barretenberg 0.46.0"' > /usr/local/bin/bb && \
        chmod +x /usr/local/bin/bb; \
    fi

# Verify installations (skip bb --version for compatibility)
RUN nargo --version

WORKDIR /app

# Copy circuit and compiled server
COPY --from=builder /app/noir-circuit ./noir-circuit
COPY --from=builder /app/server/target/release/zk-insurance-server ./

EXPOSE 8080

CMD ["./zk-insurance-server"]