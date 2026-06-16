#!/usr/bin/env bash
set -e

# Kill any leftover processes from previous runs
for port in 8080 4001 4004 5173; do
  lsof -ti:$port 2>/dev/null | xargs -r kill -9 2>/dev/null || true
done
# Wait for ports to actually be released
for port in 8080 4001 4004 5173; do
  while lsof -ti:$port &>/dev/null; do sleep 0.2; done
done

# Load .env if it exists
if [ -f .env ]; then
  set -a
  source <(grep -v '^#' .env | grep -v '^$')
  set +a
fi

# Defaults
export DATABASE_URL="${DATABASE_URL:-postgres://octa:octa_dev@localhost:5432/octa_dashboard}"
export JWT_SECRET="${JWT_SECRET:-dev_secret_change_me}"
export INITIAL_ADMIN_EMAIL="${INITIAL_ADMIN_EMAIL:-admin@example.com}"
export AGENT_URLS="${AGENT_URLS:-http://localhost:4001,http://localhost:4004}"
export RUST_LOG="${RUST_LOG:-info,dashboard_server=debug,knowledgebase_agent=debug,watcher_agent=debug}"
export SKIP_LOGIN="${SKIP_LOGIN:-true}"

# Start Postgres
if ! command -v docker &> /dev/null; then
  echo "Docker is required for local Postgres."
  exit 1
fi

echo "Starting Postgres..."
docker compose up -d postgres

echo "Waiting for Postgres..."
until docker compose exec -T postgres pg_isready -U octa -d octa_dashboard &> /dev/null; do
  sleep 1
done
echo "Postgres is ready."

# Install frontend deps if needed
if [ ! -d frontend/dashboard/node_modules ]; then
  echo "Installing frontend dependencies..."
  npm install
fi

# Build KB frontend so the agent can serve it
echo "Building knowledgebase UI..."
npm run build -w microservices/knowledgebase-agent/frontend

# Build Watcher frontend so the agent can serve it
echo "Building watcher UI..."
npm run build -w microservices/watcher-agent/frontend

export KB_STATIC_DIR="microservices/knowledgebase-agent/frontend/dist"
export WATCHER_STATIC_DIR="microservices/watcher-agent/frontend/dist"

echo ""
echo "Starting services..."
echo "  Backend:      http://localhost:8080"
echo "  KB Agent:     http://localhost:4001"
echo "  Watcher:      http://localhost:4004"
echo "  Frontend:     http://localhost:5173"
echo "  Postgres:     localhost:5432"
echo "  SKIP_LOGIN:   $SKIP_LOGIN"
echo ""

npx concurrently --kill-others \
  -n backend,kb,watcher,frontend \
  -c blue,magenta,white,green \
  "cargo run -p dashboard-server" \
  "cargo run -p knowledgebase-agent" \
  "cargo run -p watcher-agent" \
  "npm run dev -w frontend/dashboard"
