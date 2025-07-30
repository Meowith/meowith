# Stage 1: Build all binaries
FROM rust:latest AS builder
WORKDIR /app
COPY . .
RUN apt-get update && apt-get install -y libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo build --release --workspace

# Stage 2: Controller
FROM debian:bullseye-slim AS controller
COPY --from=builder /app/target/release/controller /usr/local/bin/controller
RUN apt-get update && apt-get install -y libssl-dev && rm -rf /var/lib/apt/lists/*
CMD ["controller"]

# Stage 3: Node
FROM debian:bullseye-slim AS node
COPY --from=builder /app/target/release/node /usr/local/bin/node
RUN apt-get update && apt-get install -y libssl-dev && rm -rf /var/lib/apt/lists/*
CMD ["node"]

# Stage 4: Dashboard
FROM debian:bullseye-slim AS dashboard
COPY --from=builder /app/target/release/dashboard /usr/local/bin/dashboard
RUN apt-get update && apt-get install -y libssl-dev && rm -rf /var/lib/apt/lists/*
CMD ["dashboard"]
