# Stage 1: Build all binaries
FROM rust:latest AS builder
WORKDIR /app
COPY . .
RUN apt-get update && apt-get install -y libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo build --release --workspace

# Stage 2: Controller
FROM debian:bookworm-slim AS controller
WORKDIR /app
COPY --from=builder /app/target/release/controller /app/controller
RUN apt-get update && apt-get install -y libssl-dev && rm -rf /var/lib/apt/lists/*
CMD ["./controller"]

# Stage 3: Node
FROM debian:bookworm-slim AS node
WORKDIR /app
COPY --from=builder /app/target/release/node /app/node
RUN apt-get update && apt-get install -y libssl-dev && rm -rf /var/lib/apt/lists/*
CMD ["./node"]

# Stage 4: Dashboard
FROM debian:bookworm-slim AS dashboard
WORKDIR /app
COPY --from=builder /app/target/release/dashboard /app/dashboard
RUN apt-get update && apt-get install -y libssl-dev && rm -rf /var/lib/apt/lists/*
CMD ["./dashboard"]