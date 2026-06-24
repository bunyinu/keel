# Keel — Full Handoff (rebuild from scratch)

**Version:** 0.4.1  
**Repo:** https://github.com/bunyinu/keel  
**npm:** `@keel2026/cli` (org `keel2026`, publisher `bunyinu`)  
**Cloud:** https://keel-cloud.onrender.com  
**Last updated:** 2026-06-23  
**Purpose:** Enough detail that a new engineer can **rebuild, deploy, and ship** Keel as it exists today — CLI, npm, and cloud — without oral history.

---

## Table of contents

1. [What you are rebuilding (3 artifacts)](#1-what-you-are-rebuilding-3-artifacts)
2. [Prerequisites & accounts](#2-prerequisites--accounts)
3. [Repo map](#3-repo-map)
4. [Rebuild the Rust CLI](#4-rebuild-the-rust-cli)
5. [Rebuild the npm distribution](#5-rebuild-the-npm-distribution)
6. [Rebuild Keel Cloud (server)](#6-rebuild-keel-cloud-server)
7. [Deploy Keel Cloud to Render](#7-deploy-keel-cloud-to-render) ← **deployment**
8. [Release pipeline (tag → npm + GitHub)](#8-release-pipeline-tag--npm--github)
9. [Secrets & environment variables](#9-secrets--environment-variables)
10. [End-to-end verification checklist](#10-end-to-end-verification-checklist)
11. [Cloud HTTP API](#11-cloud-http-api)
12. [SQLite schema](#12-sqlite-schema)
13. [Hook wiring (agent integration)](#13-hook-wiring-agent-integration)
14. [Product summary](#14-product-summary)
15. [Demos & proof assets](#15-demos--proof-assets)
16. [Sales narrative](#16-sales-narrative)
17. [Failed approaches (do not retry)](#17-failed-approaches-do-not-retry)
18. [Open decisions](#18-open-decisions)
19. [Key links](#19-key-links)

---

## 1. What you are rebuilding (3 artifacts)

Keel is **not one binary**. It is three shipped artifacts:

| # | Artifact | What it is | How users get it |
|---|----------|------------|------------------|
| **A** | `keel` CLI | Rust binary: goals, hooks, policy, cloud sync | `npm install -g @keel2026/cli` |
| **B** | npm packages | Node shim + 4 platform native binaries | Published on tag via GitHub Actions |
| **C** | `keel-server` | Rust Axum server + SQLite + static `web/` | Docker on Render |

**Data flow:**

```
Developer repo                    Keel Cloud (Render)
─────────────                    ───────────────────
.keel/state.json  ──push/pull──►  SQLite projects.state_json
.keel/snapshot.md                 projects.snapshot_md
.claude/settings.json             (not stored — local hooks only)
     │
     └── hooks call `keel hook …` on compact / tool / stop
```

**Core product idea (do not deviate):** Task context lives in the **git repo** (`.keel/`), not the chat. Hooks **reinject** on Claude `/compact` and **block** bad tools / premature stop. Optional cloud = fleet dashboard + sync.

---

## 2. Prerequisites & accounts

### Machine

| Tool | Version | Why |
|------|---------|-----|
| Rust | stable (2021 edition) | CLI + server |
| Node.js | 18+ | npm shim, publish scripts |
| cargo | comes with Rust | build |
| git | any | release tags |
| Docker | optional | local server smoke test |

### Accounts (production)

| Service | Used for |
|---------|----------|
| **GitHub** | Source repo, Actions release, GitHub Releases |
| **npm** | Org `@keel2026`, packages `@keel2026/cli` + 4 platform packages |
| **Render** | Host `keel-cloud` (Docker web service + persistent disk) |
| **Stripe** | Team plan payment link (env on Render) |

### npm packages to create (one-time)

If rebuilding npm from zero, create these under org `keel2026`:

- `@keel2026/cli` — main package (shim only)
- `@keel2026/linux-x64-gnu`
- `@keel2026/linux-arm64-gnu`
- `@keel2026/darwin-x64`
- `@keel2026/darwin-arm64`

Set `NPM_TOKEN` in GitHub repo secrets for automated publish.

---

## 3. Repo map

```
compo1/  (keel)
├── Cargo.toml              # version source of truth; two bins: keel, keel-server
├── src/
│   ├── main.rs             # CLI entry
│   ├── lib.rs              # module exports
│   ├── bin/keel_server.rs  # cloud server entry
│   ├── install.rs          # keel init — hooks + CLAUDE.md merge
│   ├── hooks.rs            # keel hook <event> — agent callback
│   ├── state.rs            # KeelState, KeelConfig
│   ├── snapshot.rs         # snapshot.md renderer
│   ├── policy.rs           # signed goals (ECDSA P-256 default)
│   ├── constraints.rs      # PreToolUse constraint guard
│   ├── loop_breaker.rs     # PreToolUse retry block
│   ├── acceptance.rs       # Stop hook gate
│   ├── check.rs            # keel check (CI)
│   ├── cloud.rs            # push/pull to Keel Cloud
│   ├── server/             # Axum routes + db.rs (SQLite)
│   └── …
├── web/                    # Static HTML/CSS served by keel-server
│   ├── index.html          # landing
│   ├── start.html          # sign-in / create project
│   ├── pricing.html, trust.html, team.html
│   ├── dashboard.html, dashboard-edit.html
│   ├── demo.gif            # homepage embed
│   └── site.css
├── npm/
│   ├── keel-cli/           # @keel2026/cli — bin/keel.js shim
│   └── platforms/*/        # per-OS native binary packages
├── scripts/
│   ├── stage-npm.sh        # copy release keel → npm/platforms
│   ├── release.sh          # local: test + stage + optional global install
│   └── deploy-render.sh    # trigger Render deploy via API
├── Dockerfile              # builds keel-server for Render
├── render.yaml             # Render Blueprint spec
├── .github/workflows/
│   ├── ci.yml              # PR: fmt, clippy, test, npm shim verify
│   └── release.yml         # tag v*.*.* → binaries + npm publish
└── examples/
    ├── nexus-ping-demo/    # fair compaction A/B (use this for sales)
    ├── keel-compact-demo/  # legacy demo
    └── github-keel-check.yml
```

---

## 4. Rebuild the Rust CLI

### Step 1 — Clone and build

```bash
git clone https://github.com/bunyinu/keel.git
cd keel
cargo build --release
```

Produces:

- `target/release/keel` — CLI + hooks
- `target/release/keel-server` — cloud server

### Step 2 — Run tests

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Or use the helper:

```bash
./scripts/release.sh          # test + stage npm
./scripts/release.sh --skip-tests --install-global
```

### Step 3 — Use in a project

```bash
cd /path/to/your-app
/path/to/keel/target/release/keel init
keel onboard "My task" --accept "tests pass" --constraint "no new deps"
keel config set --acceptance "npm test"
```

### What `keel init` writes

| Path | Action |
|------|--------|
| `.keel/config.json` | Defaults (loop breaker, snapshot limits) |
| `.keel/state.json` | Empty goal until `keel goal set` |
| `.keel/snapshot.md` | Generated from state |
| `.claude/settings.json` | **Merges** Keel hooks (does not delete yours) |
| `.codex/hooks.json` | Merges Keel hooks |
| `.cursor/hooks.json` | Merges Keel hooks |
| `CLAUDE.md` / `AGENTS.md` | **Appends** `## Keel` snippet if missing |

### Default `.keel/config.json` (after init)

```json
{
  "loop_breaker": { "max_same_failure": 2, "window_minutes": 60 },
  "acceptance_gate": { "enabled": false, "command": "" },
  "policy": { "mode": "off" },
  "snapshot_max_lines": 120,
  "snapshot_max_decisions": 8,
  "snapshot_max_failures": 6
}
```

---

## 5. Rebuild the npm distribution

### How it works

`@keel2026/cli` is **not** the Rust binary. It is a **Node shim** (`npm/keel-cli/bin/keel.js`) that:

1. Resolves `@keel2026/<platform>` optional dependency, OR
2. Falls back to `npm/keel-cli/vendor/keel` (local dev), OR
3. Falls back to `target/release/keel` (dev), OR
4. Uses `KEEL_BIN` env override

**Critical:** `bin/keel.js` must stay a **JavaScript shim**. Never commit a compiled ELF as `keel.js` (v0.4.0 bug).

### Stage locally

```bash
./scripts/stage-npm.sh
# copies target/release/keel → npm/platforms/<host>/bin/keel
# copies → npm/keel-cli/vendor/keel
# syncs version from Cargo.toml

node npm/keel-cli/scripts/verify-shim.js
```

### Install globally from local tree

```bash
npm install -g ./npm/keel-cli
keel --version   # must match Cargo.toml
keel policy --help
```

### Platform package layout

Each `npm/platforms/linux-x64-gnu/package.json`:

```json
{
  "name": "@keel2026/linux-x64-gnu",
  "version": "0.4.1",
  "os": ["linux"],
  "cpu": ["x64"],
  "bin": { "keel": "bin/keel" }
}
```

Only `bin/keel` (native executable) is published in platform packages.

---

## 6. Rebuild Keel Cloud (server)

### Run locally

```bash
export PORT=8080
export KEEL_DB_PATH=/tmp/keel-local.db
# optional:
export KEEL_STRIPE_PAYMENT_LINK=https://buy.stripe.com/...
export KEEL_CREATE_SECRET=my-secret-for-create
export KEEL_UPGRADE_CODES=promo1,promo2

cargo run --release --bin keel-server
```

Open http://localhost:8080

### Docker (same as Render)

```bash
docker build -t keel-server .
docker run -p 8080:8080 \
  -e KEEL_DB_PATH=/data/keel.db \
  -v keel-data:/data \
  keel-server
```

### What the server does

- Serves static pages from `web/` (embedded fallback for `demo.gif` in binary)
- SQLite at `KEEL_DB_PATH` (teams + projects)
- REST API for project create, sync, goal edit, team fleet, billing upgrade
- Health check at `GET /health` → `{"ok":true,"service":"keel-cloud"}`

### Server entry (`src/bin/keel_server.rs`)

- Reads `PORT` (Render sets this; default 8080)
- Tries `KEEL_DB_PATH`, falls back to `/tmp/keel.db` if `/data` fails
- Listens `0.0.0.0:PORT`

---

## 7. Deploy Keel Cloud to Render

This is the **production deployment path**. CLI/npm do **not** auto-deploy; only the server runs on Render.

### Architecture on Render

```
GitHub push (main) ──► Render Web Service "keel-cloud"
                         runtime: docker
                         Dockerfile → keel-server
                         disk: 1GB mounted at /data
                         SQLite: /data/keel.db
                         health: GET /health
                         URL: https://keel-cloud.onrender.com
```

### Files involved

| File | Role |
|------|------|
| [`render.yaml`](render.yaml) | Blueprint: service name, env vars, disk, health check |
| [`Dockerfile`](Dockerfile) | Multi-stage Rust build → debian-slim + `keel-server` + `web/` |
| [`scripts/deploy-render.sh`](scripts/deploy-render.sh) | API trigger for redeploy |

### First-time deploy (Blueprint)

1. Push repo to GitHub (`bunyinu/keel` or your fork).
2. Log in to https://dashboard.render.com
3. **New → Blueprint** → connect GitHub repo
4. Render reads `render.yaml` and creates:
   - Web service `keel-cloud`
   - Docker build from `Dockerfile`
   - Persistent disk `keel-data` → `/data`
5. In Render dashboard → **Environment**, set secrets (see [§9](#9-secrets--environment-variables)):
   - `KEEL_STRIPE_PAYMENT_LINK`
   - `KEEL_UPGRADE_CODES`
   - `KEEL_CREATE_SECRET`
6. Wait for deploy. Verify:
   ```bash
   curl https://keel-cloud.onrender.com/health
   ```

### Redeploy after code changes

**Option A — Git auto-deploy (recommended):**  
Push to `main` → Render rebuilds Docker image.

**Option B — API script:**

```bash
export RENDER_API=rnd_xxxxxxxxxxxx
# optional: export RENDER_OWNER_ID=tea-xxxxx
# optional: export RENDER_SERVICE_NAME=keel-cloud
./scripts/deploy-render.sh
```

Script behavior:

- If service exists → `POST /v1/services/{id}/deploys`
- If not → prints Blueprint instructions

### Dockerfile notes (why it looks weird)

- **Runs as root** so Render persistent disk at `/data` is writable
- `mkdir -p /data` in image
- `COPY web` for static assets
- Only builds `--bin keel-server` (not `keel` CLI)

### Render free tier caveats

- Cold starts on free plan
- Single instance + SQLite — not HA
- Disk persists across deploys; backup `keel.db` before risky migrations

### Connect a local repo to cloud (after deploy)

```bash
# On website: https://keel-cloud.onrender.com/start → create project → copy id + api_key

keel cloud link \
  --url https://keel-cloud.onrender.com \
  --project YOUR_PROJECT_ID \
  --key YOUR_API_KEY

keel cloud push
```

Creates `.keel/cloud.json` (usually gitignored).

---

## 8. Release pipeline (tag → npm + GitHub)

### Trigger

```bash
# bump version in Cargo.toml first (source of truth)
git commit -am "Release v0.4.2"
git tag v0.4.2
git push origin main
git push origin v0.4.2
```

### GitHub Actions (`.github/workflows/release.yml`)

On tag `v*.*.*`:

1. **Matrix build** (4 targets):
   - `x86_64-unknown-linux-gnu` → `@keel2026/linux-x64-gnu`
   - `aarch64-unknown-linux-gnu` → `@keel2026/linux-arm64-gnu`
   - `x86_64-apple-darwin` → `@keel2026/darwin-x64`
   - `aarch64-apple-darwin` → `@keel2026/darwin-arm64`
2. `./scripts/stage-npm.sh --target … --npm-pkg …`
3. Upload artifacts
4. **publish job:**
   - Merge platform packages
   - `node npm/keel-cli/scripts/sync-version.js $VERSION`
   - GitHub Release with tarballs
   - `node npm/keel-cli/scripts/prep-publish.js $VERSION`
   - `npm publish` each platform package + `@keel2026/cli`

### Required GitHub secret

| Secret | Purpose |
|--------|---------|
| `NPM_TOKEN` | npm publish (skip if unset — workflow logs warning) |

### Post-release verification (mandatory)

```bash
npm install -g @keel2026/cli@0.4.2
which keel
keel --version          # must show 0.4.2
file $(which keel)      # must be node script or symlink to it — NOT ELF
keel policy --help      # must exist on 0.4+
```

### CI on every PR (`.github/workflows/ci.yml`)

- `cargo fmt`, `clippy`, `test`, `release build`
- `./scripts/stage-npm.sh` + `verify-shim.js`

**Cloud is NOT deployed by CI.** Deploy server separately via Render.

---

## 9. Secrets & environment variables

### Keel Cloud (Render dashboard)

| Variable | Required | Example | Purpose |
|----------|----------|---------|---------|
| `PORT` | Auto | `10000` | Set by Render |
| `KEEL_DB_PATH` | Yes | `/data/keel.db` | SQLite path on persistent disk |
| `RUST_LOG` | No | `info` | Logging |
| `KEEL_FREE_PROJECT_LIMIT` | No | `1` | Free tier project cap |
| `KEEL_PRO_PROJECT_LIMIT` | No | `50` | Team tier project cap |
| `KEEL_STRIPE_PAYMENT_LINK` | For billing | `https://buy.stripe.com/...` | Pricing page CTA |
| `KEEL_UPGRADE_CODES` | For billing | `code1,code2` | Redeem after Stripe payment |
| `KEEL_CREATE_SECRET` | Recommended | random string | `POST /api/projects` requires header `X-Keel-Create-Secret` |

In `render.yaml`, billing/create secrets use `sync: false` — you set them manually in Render UI.

### Local CLI

| Variable | Purpose |
|----------|---------|
| `KEEL_BIN` | Override binary path in installed hooks |

### Local server dev

Same as Render vars; use `/tmp/keel.db` if no disk.

---

## 10. End-to-end verification checklist

Run this after any rebuild or deploy. Every step should pass.

### CLI

```bash
keel --version
keel doctor
mkdir /tmp/keel-smoke && cd /tmp/keel-smoke
git init && keel init
keel goal set "smoke test" --accept "ok"
test -f .keel/snapshot.md
test -f .claude/settings.json
rg "keel hook" .claude/settings.json
keel check
```

### Hooks (manual)

```bash
keel hook session-start --agent claude < /dev/null
# should print snapshot text
```

### Server local

```bash
curl -s localhost:8080/health | jq .
curl -s -o /dev/null -w "%{http_code}" localhost:8080/pricing   # 200
```

### Server production

```bash
curl -s https://keel-cloud.onrender.com/health
curl -s -o /dev/null -w "%{http_code}" https://keel-cloud.onrender.com/demo.gif
```

### npm shim (after publish)

```bash
npm install -g @keel2026/cli@latest
keel --version
keel policy verify   # in a repo with policy
```

### Compaction demo (proof)

```bash
bash examples/nexus-ping-demo/demo.sh
# without-keel: no port 7429
# with-keel: port 7429 after /compact
```

---

## 11. Cloud HTTP API

Base URL: `https://keel-cloud.onrender.com`

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| GET | `/health` | none | Health check |
| GET | `/`, `/pricing`, `/trust`, `/start`, … | none | Static HTML |
| GET | `/demo.gif` | none | Demo asset |
| POST | `/api/teams` | none | Create team |
| POST | `/api/projects` | `X-Keel-Create-Secret` if configured | Create project → returns `id`, `api_key` |
| GET | `/api/projects/{id}` | `Bearer {api_key}` | Get project state |
| POST | `/api/projects/{id}/sync` | Bearer | Push `state` + `snapshot` |
| PUT | `/api/projects/{id}/goal` | Bearer | Web goal editor |
| POST | `/api/teams/projects/link` | team license | Link project to team |
| GET | `/api/teams/projects` | `?license=` | Fleet list |
| POST | `/api/billing/upgrade` | body: `team_license`, `code` | Free → Pro |

CLI sync implementation: `src/cloud.rs` (`push_state`, `pull_state`).

---

## 12. SQLite schema

Created in `src/server/db.rs` on first boot:

```sql
CREATE TABLE teams (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    plan TEXT NOT NULL DEFAULT 'free',      -- 'free' | 'pro'
    license_key TEXT NOT NULL UNIQUE,
    max_projects INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    api_key TEXT NOT NULL UNIQUE,           -- keel_{uuid}
    team_id TEXT,
    state_json TEXT NOT NULL DEFAULT '{}',
    snapshot_md TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL
);
```

Limits: `KEEL_FREE_PROJECT_LIMIT` (default 1), `KEEL_PRO_PROJECT_LIMIT` (default 50).

---

## 13. Hook wiring (agent integration)

Installed into `.claude/settings.json` by `keel init`:

| Event | Matcher | Command |
|-------|---------|---------|
| PreCompact | all | `keel hook pre-compact --agent claude` |
| SessionStart | `compact\|resume` | `keel hook session-start --agent claude` |
| PreToolUse | Bash, Edit, Write, ApplyPatch | `keel hook pre-tool-use --agent claude` |
| PostToolUse | same | `keel hook post-tool-use --agent claude` |
| Stop | all | `keel hook stop --agent claude` |

**PreCompact** prints JSON `systemMessage` with full `snapshot.md` — this is how goals survive Claude `/compact`.

**PreToolUse** can return `decision: block` (loop breaker, constraints, signed policy).

**Stop** runs acceptance gate shell command; exit 2 blocks session end (Claude).

Codex: same events in `.codex/hooks.json` — user must `/hooks` trust once.  
Cursor: `.cursor/hooks.json` — less battle-tested.

---

## 14. Product summary

### What Keel is

Repo-local **task ticket** (goal, acceptance, constraints, progress, failures) + **hook-layer enforcement** across Claude Code, Codex, Cursor.

### What Keel is not

- Replacement for `CLAUDE.md` (house rules stay in your md; `keel init` appends a small Keel section)
- Replacement for Claude Tasks API or Agentpack
- Bulletproof vs prompt injection

### vs “good CLAUDE.md + skills + loop”

| | md + skills | Keel |
|--|-------------|------|
| Survives `/compact` | Only if your loop re-reads files | PreCompact injects snapshot automatically |
| Block bad commands | Advisory | PreToolUse deny |
| Block “done” with failing tests | Advisory | Stop hook |
| CI signed goal | DIY | `keel policy` + `keel check` |

### Pricing

| Tier | Price |
|------|-------|
| Free CLI + 1 cloud project | $0 |
| Team (fleet, 50 repos) | $15/mo |

---

## 15. Demos & proof assets

### Primary — `examples/nexus-ping-demo/` (fair baseline)

Both arms have `CLAUDE.md` + `.claude/`. Only difference: `keel init` or not.

| Arm | After Claude `/compact` |
|-----|-------------------------|
| without-keel | `process.env.PORT` — cannot ship secret **7429** |
| with-keel | **`PORT = 7429`**, correct JSON |

```bash
bash examples/nexus-ping-demo/demo.sh
bash examples/nexus-ping-demo/record.sh   # asciinema + GIF
```

Artifacts: `demo.gif`, `demo.cast`, `artifacts/results/`, `RESULTS.md`  
Homepage: `web/demo.gif`

### Legacy — `examples/keel-compact-demo/`

Port 8842 vs 3000; without-keel had no `.claude` (less fair).

### CI example

`examples/github-keel-check.yml` — run `keel check` on PR.

---

## 16. Sales narrative

**Wedge:** *Goal survives Claude `/compact`; CI enforces it.*

**One sentence:** Keel Team is the control plane for AI agents in your repos — see every goal, gate merges with `keel check`, same guardrails in Claude, Codex, and Cursor.

**Buyer:** Eng lead, 3–15 devs, multiple repos, Claude Code or Codex.

**Proof:** Run nexus-ping demo or show `demo.gif`.

**Show HN draft:** `docs/SHOW_HN.md`

---

## 17. Failed approaches (do not retry)

| Approach | Why |
|----------|-----|
| Commit ELF as `npm/keel-cli/bin/keel.js` | v0.4.0 shipped wrong version, no `policy` cmd |
| `claude --bare` in demos | Breaks Claude auth |
| without-keel with no `.claude` | Unrealistic baseline |
| Global hooks without per-repo `keel init` | Empty snapshots |
| `npm test` as acceptance gate before tests pass | Infinite fail |
| asciinema without `--overwrite` | Re-record aborts |
| Expect GHA to deploy cloud | Only Render deploys server |

---

## 18. Open decisions

1. Cursor: first-class vs documented manual hooks?
2. Product telemetry vs privacy-blind?
3. Windows npm platform package?
4. Retire greet-api demo in favor of nexus-ping only?
5. Add “power user md + loop” third demo arm?

---

## 19. Key links

| Resource | URL / path |
|----------|------------|
| GitHub | https://github.com/bunyinu/keel |
| Cloud | https://keel-cloud.onrender.com |
| npm | `@keel2026/cli` |
| Handoff (this file) | `docs/HANDOFF.md` |
| Deploy blueprint | `render.yaml` |
| Deploy script | `scripts/deploy-render.sh` |
| Docker | `Dockerfile` |
| Release workflow | `.github/workflows/release.yml` |
| Fair demo | `examples/nexus-ping-demo/` |
| Show HN | `docs/SHOW_HN.md` |

---

## Quick rebuild order (TL;DR for a new engineer)

1. `cargo test && cargo build --release` — CLI works  
2. `./scripts/stage-npm.sh && npm install -g ./npm/keel-cli` — npm works  
3. `cargo run --release --bin keel-server` — cloud works locally  
4. Push to GitHub → Render Blueprint from `render.yaml` — cloud live  
5. Set Render secrets (Stripe, upgrade codes, create secret)  
6. `git tag vX.Y.Z && git push origin vX.Y.Z` — npm published  
7. `npm install -g @keel2026/cli@X.Y.Z && keel --version` — verify shim  
8. `bash examples/nexus-ping-demo/demo.sh` — verify product proof  

---

*End of handoff. If something fails, start at §10 verification checklist and trace which artifact (CLI / npm / server) broke.*
