# Chef stage - compute recipe for dependencies
FROM rust:latest AS chef
RUN cargo install cargo-chef
WORKDIR /app

# Planner stage - analyze dependencies
FROM chef AS planner

# Copy workspace files needed for dependency analysis
COPY Cargo.toml Cargo.lock ./
COPY solver/Cargo.toml solver/Cargo.toml
COPY solver/build.rs solver/build.rs
COPY solver/proto solver/proto
COPY svm/programs/order_book/Cargo.toml svm/programs/order_book/Cargo.toml
COPY deployments/quoter/Cargo.toml deployments/quoter/Cargo.toml
COPY deployments/substreams/Cargo.toml deployments/substreams/Cargo.toml

# Create stub source files for workspace members
RUN mkdir -p solver/src && echo "fn main() {}" > solver/src/main.rs
RUN mkdir -p svm/programs/order_book/src && echo "" > svm/programs/order_book/src/lib.rs
RUN mkdir -p deployments/quoter/src && echo "fn main() {}" > deployments/quoter/src/main.rs
RUN mkdir -p deployments/substreams/src && echo "" > deployments/substreams/src/lib.rs

RUN cargo chef prepare --recipe-path recipe.json

# Builder stage - compile dependencies (cached) then source
FROM chef AS builder

# Install protobuf compiler for gRPC builds
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

# Copy the recipe and build dependencies only (this layer is cached!)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --package solver

# Now copy actual source code
COPY Cargo.toml Cargo.lock ./
COPY solver ./solver
COPY svm ./svm
COPY deployments/quoter ./deployments/quoter
COPY deployments/substreams ./deployments/substreams
# EVM contract ABIs are needed at compile time for alloy sol! macro
COPY evm/out ./evm/out

# Build solver binary (only application code compiles now)
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
