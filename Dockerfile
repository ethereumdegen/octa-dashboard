# Dashboard server — main backend + admin UI
# --- Stage 1: Build frontend ---
FROM node:20-alpine AS frontend-build
WORKDIR /app
COPY package.json package-lock.json* ./
COPY frontend/dashboard/package.json frontend/dashboard/
COPY microservices/knowledgebase-agent/frontend/package.json microservices/knowledgebase-agent/frontend/
RUN npm ci
COPY frontend/dashboard/ frontend/dashboard/
ARG VITE_GITHUB_CLIENT_ID
ARG VITE_GITHUB_REDIRECT_URI
RUN npm run build -w frontend/dashboard

# --- Stage 2: Build Rust binary ---
FROM rust:1.91-bookworm AS rust-build
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY crates/ crates/
COPY backend/ backend/
COPY microservices/ microservices/
RUN cargo build --release -p dashboard-server

# --- Stage 3: Runtime ---
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=rust-build /app/target/release/dashboard-server /app/dashboard-server
COPY --from=frontend-build /app/frontend/dashboard/dist /app/static
EXPOSE 8080
CMD ["/app/dashboard-server"]
