# Octa Dashboard

An open-source admin dashboard with a pluggable microservice architecture. Ships with a **knowledgebase agent** (a document-RAG app — upload documents and chat with an agent over them) as a sample microservice — add your own by following the agent protocol.

```
                    Internet
                       |
                [dashboard:8080]  ← only public service
                    /     \
    [kb-agent:4001]   [your-agent:400X]
                    \     /
                [PostgreSQL 16]
```


<img width="1352" height="914" alt="image" src="https://github.com/user-attachments/assets/b750c4e9-6890-41cb-b416-5861d4ae2255" />


## Features

- Team management with GitHub OAuth login
- Automatic microservice discovery and health monitoring
- Proxy layer — microservice UIs are served through the dashboard
- API key management (project-scoped tokens)
- Platform secrets (securely share config with agents)
- Analytics and usage tracking
- Mobile-responsive UI

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (1.91+)
- [Node.js](https://nodejs.org/) (20+)
- [Docker](https://www.docker.com/) (for PostgreSQL)

### Run locally

```bash
cp .env.template .env
./dev.sh
```

This starts PostgreSQL, the dashboard server (`:8080`), the knowledgebase agent (`:4001`), and the frontend dev server (`:5173`). With `SKIP_LOGIN=true` (the default), auth is bypassed for development.

Open **http://localhost:5173** to access the dashboard.

### Manual setup

```bash
# Start Postgres
docker compose up -d

# Install JS dependencies
npm install

# Build the knowledgebase frontend
npm run build -w microservices/knowledgebase-agent/frontend

# Start all services (in separate terminals)
cargo run -p dashboard-server
KB_STATIC_DIR=microservices/knowledgebase-agent/frontend/dist cargo run -p knowledgebase-agent
npm run dev -w frontend/dashboard
```

---

## Architecture

### Dashboard Server (`backend/`)

The central hub. Handles auth, team management, API keys, analytics, migrations, and proxies requests to microservices. Built with Axum (Rust).

### Microservices (`microservices/`)

Each microservice is an independent Rust binary with its own optional frontend. The dashboard discovers them via `/.well-known/agent.json` and health-checks them every 30 seconds.

**Included:** `knowledgebase-agent` — a document-RAG knowledgebase. Organize documents into knowledgebases and folders, upload files to S3, index them, and chat with a retrieval-augmented agent over their contents. Upload/indexing/chat require an OpenAI API key + an S3-compatible store; without them the agent still runs (those features are simply disabled).

### Agent Protocol (`crates/agent-protocol/`)

Shared library that defines the manifest format and common utilities for building agents.

### Frontend (`frontend/dashboard/`)

React + TypeScript + Tailwind + Radix UI. Communicates with the backend API and renders microservice UIs via iframe proxy.

---

## Adding a New Microservice

1. Create a new directory under `microservices/your-agent/`
2. Implement the agent protocol:
   - `GET /.well-known/agent.json` — returns a manifest with name, description, icon, version
   - `GET /health` — returns 200 when healthy
   - `GET /api/*` — your API routes (protected by `AGENT_SECRET` header)
   - Optionally serve a frontend UI from `/ui/`
3. Add it to `Cargo.toml` workspace members
4. If it has a frontend, add to `package.json` workspaces
5. Set `AGENT_URLS` to include your agent's URL, or add it via Settings > Agent Sources in the UI

The dashboard will auto-discover, health-check, and proxy your agent.

---

## Deploy to Railway

Octa Dashboard runs as **2+ services + 1 Postgres database** on [Railway](https://railway.app).

### 1. Create the Project

1. Create a new project on Railway
2. Add a **PostgreSQL** plugin (Railway provisions it and provides `DATABASE_URL`)

### 2. Generate Secrets

```bash
# JWT secret for user sessions
openssl rand -hex 32

# Agent secret for inter-service auth
openssl rand -hex 32
```

### 3. Create a GitHub OAuth App

1. GitHub > Settings > Developer settings > OAuth Apps > New
2. **Homepage URL**: `https://<your-railway-domain>`
3. **Callback URL**: `https://<your-railway-domain>/api/auth/github/callback`
4. Note the **Client ID** and **Client Secret**

### 4. Create Services

Create services from the same GitHub repo. Set the **Dockerfile Path** in each service's Build settings.

#### Service: `dashboard`

| Setting | Value |
|---------|-------|
| Dockerfile Path | `Dockerfile` |
| Health Check | `/api/agents` |

**Environment variables:**

```
DATABASE_URL         = ${{Postgres.DATABASE_URL}}
JWT_SECRET           = <from step 2>
AGENT_SECRET         = <from step 2>
GITHUB_CLIENT_ID     = <from step 3>
GITHUB_CLIENT_SECRET = <from step 3>
GITHUB_REDIRECT_URI  = https://<your-domain>/api/auth/github/callback
INITIAL_ADMIN_EMAIL  = you@example.com
AGENT_URLS           = http://${{knowledgebase-agent.RAILWAY_PRIVATE_DOMAIN}}:4001
RUST_LOG             = info
```

> `AGENT_URLS` seeds the DB on first boot. After that, manage agents from Settings > Agent Sources in the UI.

#### Service: `knowledgebase-agent`

| Setting | Value |
|---------|-------|
| Dockerfile Path | `microservices/knowledgebase-agent/Dockerfile` |
| Health Check | `/health` |

**Environment variables:**

```
DATABASE_URL   = ${{Postgres.DATABASE_URL}}
AGENT_SECRET   = <same as dashboard>
DASHBOARD_URL  = http://${{dashboard.RAILWAY_PRIVATE_DOMAIN}}:8080
PORT           = 4001
RUST_LOG       = info
```

> `KB_STATIC_DIR` is baked into the Dockerfile as `/app/static`.
>
> To enable document upload + RAG chat, add an `OPENAI_API_KEY` and S3 credentials (`S3_REGION`, `S3_ACCESS_KEY`, `S3_SECRET_KEY`, `S3_BUCKET`, and `S3_ENDPOINT` for non-AWS stores) — either as env vars here or as Platform Secrets in the dashboard (the agent fetches missing ones via `DASHBOARD_URL`).

#### Adding more microservices

For each additional agent, create another Railway service pointing to its Dockerfile, give it the shared `DATABASE_URL` and `AGENT_SECRET`, and add its private URL to `AGENT_URLS` on the dashboard (or use the Agent Sources UI).

### 5. Networking

1. **Dashboard** service > Settings > Networking > **Generate Domain**
2. Update `GITHUB_REDIRECT_URI` to match
3. **Do NOT** generate public domains for microservices — they communicate over Railway's private network

### 6. Deploy

Push to your repo. Railway builds all services from their Dockerfiles. First deploy takes ~5 minutes (Rust compilation). Subsequent deploys use Docker layer caching.

Railway builds in parallel. The dashboard may start before agents are ready — that's fine. Agents auto-register once healthy (checked every 30s).

### 7. Verify

1. Open your Railway domain
2. Log in with GitHub (must match `INITIAL_ADMIN_EMAIL`)
3. **Worker Agents** — your agents should show as healthy
4. Try the Knowledgebase page

---

## Shared Variables (recommended)

Use Railway's **Shared Variables** to avoid duplication:

| Variable | Scope |
|----------|-------|
| `DATABASE_URL` | All services |
| `AGENT_SECRET` | All services |
| `RUST_LOG` | All services |

---

## How Services Communicate

```
Browser → dashboard (public)
  → /api/*                    handled by dashboard backend
  → /api/agents/*/proxy/*     proxied to microservice (private network + AGENT_SECRET)

dashboard → microservices (private network)
  → /.well-known/agent.json   discovery (every 30s)
  → /health                   health check (every 30s)
  → /api/*                    proxied from browser (requires AGENT_SECRET)
```

## Security Model

- **AGENT_SECRET**: Shared secret between dashboard and all agents. Sent as `Authorization: Bearer <secret>`. Health and manifest endpoints are public (needed for Railway health checks).
- **JWT**: User sessions use signed JWTs in cookies. 7-day expiry.
- **API Keys**: Project-scoped tokens (prefix `tk_`) for programmatic access. Stored as SHA-256 hashes.
- **GitHub OAuth**: Login gate. Only emails in the `team_members` table can access the dashboard.

## Database

All services share a single PostgreSQL database. Migrations run automatically on dashboard startup. Microservices manage their own tables.

---

## Environment Variable Reference

### Dashboard

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | Yes | — | PostgreSQL connection string |
| `JWT_SECRET` | Yes | — | JWT signing secret |
| `GITHUB_CLIENT_ID` | No | `""` | GitHub OAuth client ID |
| `GITHUB_CLIENT_SECRET` | No | `""` | GitHub OAuth secret |
| `GITHUB_REDIRECT_URI` | No | `""` | OAuth callback URL |
| `INITIAL_ADMIN_EMAIL` | No | `""` | Email to seed as first admin |
| `AGENT_URLS` | No | `""` | Comma-separated agent URLs (seeds on first boot) |
| `AGENT_SECRET` | No | `""` | Shared secret for agent auth |
| `SKIP_LOGIN` | No | `false` | Bypass auth (dev only) |
| `PORT` | No | `8080` | Listen port |
| `RUST_LOG` | No | `info` | Log level |

### Knowledgebase Agent

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | Yes | — | PostgreSQL connection string |
| `AGENT_SECRET` | No | `""` | Shared secret |
| `PORT` | No | `4001` | Listen port |
| `KB_STATIC_DIR` | No | `static` | Frontend assets path |
| `DASHBOARD_URL` | No | `http://localhost:8080` | Dashboard URL (used to fetch unset secrets from Platform Secrets) |
| `OPENAI_API_KEY` | No | `""` | Enables indexing + the RAG chat agent |
| `OPENAI_MODEL` | No | `gpt-5.4` | Chat model |
| `S3_REGION` | No | `nyc3` | S3 region |
| `S3_ACCESS_KEY` | No | `""` | Enables document upload/indexing |
| `S3_SECRET_KEY` | No | `""` | Enables document upload/indexing |
| `S3_BUCKET` | No | `knowledgebase-docs` | Bucket for uploaded documents |
| `S3_ENDPOINT` | No | `""` | Custom endpoint for S3-compatible stores |
| `RUST_LOG` | No | `info` | Log level |

> The OpenAI + S3 settings can be provided as env vars, or stored once as **Platform Secrets** in the dashboard (Settings) — the agent fetches any it's missing from `DASHBOARD_URL` using `AGENT_SECRET`. Without them the agent runs but upload/indexing/chat are disabled.

---

## Troubleshooting

**Agents show as unhealthy**
- Check agent logs
- Verify `AGENT_URLS` or Agent Sources point to the correct address
- Verify `AGENT_SECRET` matches across services
- Wait 30s for health check cycle

**Login fails**
- Verify `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET`, `GITHUB_REDIRECT_URI`
- Redirect URI must exactly match GitHub OAuth app settings
- User's GitHub email must be in `team_members` (seed via `INITIAL_ADMIN_EMAIL`)

**Slow first build**
- First Rust build compiles all dependencies (~5 min). Subsequent builds use Docker layer caching.

---

## License

MIT
