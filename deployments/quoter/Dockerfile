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

# Build quoter binary
RUN cargo build --release --package quoter

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/quoter /app/quoter

# Copy config files
COPY deployments/config/quoter-*.yaml /app/config/

# Environment defaults
ENV QUOTER_ENV=development
ENV QUOTE_TIMEOUT_MS=500
ENV GRPC_HOST=0.0.0.0
ENV GRPC_PORT=50051
ENV API_PORT=3000

EXPOSE 50051 3000

CMD ["/app/quoter"]
