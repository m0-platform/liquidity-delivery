# Build stage
FROM rust:latest AS builder

WORKDIR /app

# Install protobuf compiler for gRPC builds
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY deployments/quoter ./deployments/quoter
COPY solver ./solver
COPY svm ./svm
# EVM contract ABIs are needed at compile time for alloy sol! macro
COPY evm/out ./evm/out

# Build solver binary
RUN cargo build --release --package solver

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/solver /app/solver

# Copy config files
COPY deployments/config/solver-*.yaml /config/

CMD ["/app/solver"]
