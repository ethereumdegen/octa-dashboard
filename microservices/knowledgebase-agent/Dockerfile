# Build from repo root: docker build -f microservices/knowledgebase-agent/Dockerfile .

# --- Stage 1: Build frontend ---
FROM node:20-alpine AS frontend-build
WORKDIR /app
COPY package.json package-lock.json* ./
COPY frontend/dashboard/package.json frontend/dashboard/
COPY microservices/knowledgebase-agent/frontend/package.json microservices/knowledgebase-agent/frontend/
COPY microservices/security-agent/frontend/package.json microservices/security-agent/frontend/
COPY microservices/pool-security-agent/frontend/package.json microservices/pool-security-agent/frontend/
RUN npm ci
COPY microservices/knowledgebase-agent/frontend/ microservices/knowledgebase-agent/frontend/
RUN npm run build -w microservices/knowledgebase-agent/frontend

# --- Stage 2: Build Rust binary ---
FROM rust:1.91-bookworm AS rust-build
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY crates/ crates/
COPY backend/ backend/
COPY microservices/ microservices/
RUN cargo build --release -p knowledgebase-agent

# --- Stage 3: Runtime ---
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=rust-build /app/target/release/knowledgebase-agent /app/knowledgebase-agent
COPY --from=frontend-build /app/microservices/knowledgebase-agent/frontend/dist /app/static
ENV KB_STATIC_DIR=/app/static
EXPOSE 4001
CMD ["/app/knowledgebase-agent"]
