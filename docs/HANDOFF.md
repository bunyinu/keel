# Keel — Project Handoff

**Version:** 0.3.1 (Rust CLI + npm shim + Keel Cloud on Render)  
**Repo:** `compo1` (Keel)  
**Last updated:** 2026-06-22  
**Audience:** Next owner, co-founder, or contractor taking Keel to market

---

## 1. Executive summary

**Keel is repo-local agent state for Claude Code and Codex** — a goal file in `.keel/` that survives compaction, plus hooks in `.claude/` / `.codex/` that re-inject it and optionally **block** bad tool use (loop breaker, constraint guard) and **block session end** until acceptance tests pass.

**Core idea (do not deviate):** Task context lives in the **git repo**, not the chat transcript. Same state across Claude and Codex. Enforcement, not reminders.

**Business model:**

| Tier | Price | Value |
|------|-------|-------|
| Free | $0 | Local CLI + 1 cloud project |
| Team | $15/mo | Fleet dashboard, 50 repos, `keel check` in CI |

**Live:** https://keel-cloud.onrender.com

**What’s proven:** A clean A/B compaction demo shows Claude **keeps port 8842 with Keel** and **defaults to 3000 without** after forced `/compact`. Artifacts: `examples/keel-compact-demo/`.

**What’s not proven:** Paid conversion, retention, or that teams will adopt repo-owned goals as process.

---

## 2. The two directories (critical)

New owners and users confuse these constantly. **Both are required for the full loop.**

| | `.keel/` | `.claude/` / `.codex/` |
|--|----------|-------------------------|
| **Role** | **Memory** — durable state | **Wiring** — when to run Keel |
| **Created by** | `keel init` | `keel init` (merges hooks) |
| **Key files** | `state.json`, `snapshot.md`, `config.json`, `attempts.jsonl` | `settings.json` / `hooks.json` |
| **Without the other** | Goal on disk; nothing restores it after compact | Hooks run; snapshot says “No active goal” |

```
User sets goal → .keel/state.json + snapshot.md
                        ↑
Compaction clears chat ─┘
                        │
.claude hooks → keel hook pre-compact / session-start → inject snapshot
```

**Anti-pattern:** Global Keel hooks in `~/.claude/settings.json` without `keel init` + goal in each repo → empty snapshots, false sense of “using Keel.”

---

## 3. Architecture map

```
npm (@keel2026/cli)          Rust binaries
        │                      ├── keel (CLI + hooks)
        └──────────────────────└── keel-server (Axum + SQLite)
                                         │
                                         ▼
                              Keel Cloud (Render)
                              /, /pricing, /team, /dashboard/:id
```

### Rust modules (`src/`)

| Module | Purpose |
|--------|---------|
| `main.rs` | CLI: init, goal, progress, tui, check, cloud, doctor, hook |
| `install.rs` | `keel init` — `.keel/`, merge hooks, CLAUDE.md snippets |
| `hooks.rs` | Hook entrypoint (pre/post compact, session-start, pre/post tool, stop) |
| `state.rs` | `KeelState`, `KeelConfig`, persistence |
| `snapshot.rs` | Renders `snapshot.md` (goal, progress, decisions, do-not-retry) |
| `constraints.rs` | PreToolUse: no deps, read-only, banned keywords |
| `loop_breaker.rs` | Blocks repeated identical failures |
| `acceptance.rs` | Stop hook: run shell command before agent ends |
| `check.rs` | CI gate: init + goal + acceptance (+ optional cloud ping) |
| `cloud.rs` | push/pull to Keel Cloud |
| `goal_edit.rs` | Shared `GoalForm` (CLI / TUI / web API) |
| `tui.rs` | ratatui goal editor |
| `doctor.rs` | Diagnostics |
| `server/` | Teams, projects, billing upgrade, dashboards |

### Distribution

- **Users:** `npm install -g @keel2026/cli`
- **Release:** tag `v*.*.*` → GitHub Actions → platform binaries + npm publish
- **Contributors:** `cargo test`, `./scripts/stage-npm.sh`, `./scripts/release.sh --install-global`

### Data layout (per repo)

```
.keel/
  state.json        # goal, progress, decisions, compaction/session counts
  snapshot.md       # injected into agent context
  attempts.jsonl    # loop breaker log
  changelog.jsonl   # audit events
  config.json       # loop breaker thresholds, acceptance gate
  cloud.json        # optional cloud link
```

---

## 4. Current product state (v0.3)

### Shipped (tagged v0.3.1)

- Rust CLI with sub-ms hooks
- npm global install (linux/mac, x64/arm64)
- Claude Code + Codex hook installers
- Constraint guard, loop breaker, acceptance gate (Stop)
- TUI goal editor
- Keel Cloud: project create, sync, web goal edit, team fleet, Stripe upgrade codes
- `keel doctor`, `keel update`

### In working tree (not necessarily released)

- `keel check` + `examples/github-keel-check.yml` (CI gate)
- Fleet fields on team dashboard (`goal_title`, `current_step` in `server/db.rs`)
- Compaction demo (`examples/keel-compact-demo/`)
- README v0.3 positioning vs Tasks API / Agentpack

### Known rough edges

- Keel goal in this repo is still a placeholder (“Do not deviate form the core idea”)
- Stripe payment link can be placeholder env on server
- `npm test` failed once in attempts log — do not blindly retry as acceptance gate
- Partial `.keel/` dirs possible (e.g. only `attempts.jsonl`) if hooks ran without `keel init`
- No Cursor hook path (large distribution gap)
- Global vs project hook story is confusing; doctor should warn harder

---

## 5. Competitive position

| | **Keel** | **Claude Tasks API** | **Agentpack** |
|--|----------|----------------------|---------------|
| State location | `.keel/` in repo (git) | `~/.claude/tasks/` | `.agentpack/` |
| Agents | Claude + Codex | Claude only | Any MCP client |
| Tool blocks | Yes | No | No |
| Stop gate (tests) | Yes | No | No |
| Hosted team UI | Yes ($15) | No | No |

**When Keel wins:** Repo-owned, enforceable goal shared across Claude *and* Codex, CI gate, team dashboard.

**When Keel loses:** Claude-only shops that want native task graphs → Tasks API (v2.1.16+). Rich ledger / MCP-first → Agentpack.

**Strategic line:** Keel complements; it does not replace `CLAUDE.md` or full task systems.

---

## 6. Market goal: 1% of Claude Code + Codex users

### What “1%” means (honest math)

Public MAU for Claude Code and Codex CLI is **not disclosed**. Planning ranges:

| Assumption | Combined active agent-in-terminal users | **1% install base** | **5% of those on Team ($15)** |
|------------|----------------------------------------|---------------------|-------------------------------|
| Conservative | 200,000 | 2,000 | 100 → **$1.5k MRR** |
| Mid | 500,000 | 5,000 | 250 → **$3.75k MRR** |
| Aggressive | 2,000,000 | 20,000 | 1,000 → **$15k MRR** |

**1% distribution** is an **install / init** target, not revenue. Revenue needs:

1. **Init** (`keel init` in a real repo)
2. **Goal set** (otherwise product is inert)
3. **Habit** (hooks on every session)
4. **Upgrade trigger** (2+ repos, CI, or manager visibility)

**Realistic year-1 success:** 2k–5k inits, 200–500 paying teams, one public case study — not 20k paid seats.

---

## 7. What’s missing to stand the market

### A. Product (must-have for 1%)

| Gap | Why it blocks adoption | Priority |
|-----|------------------------|----------|
| **Cursor / IDE hooks** | Huge share of “agent dev” is Cursor, not only Claude Code/Codex CLI | P0 |
| **One-command onboarding** | `keel init` + goal should be one guided flow; empty goal = broken | P0 |
| **Partial `.keel` detection** | `doctor` must fail if only `attempts.jsonl` exists | P1 |
| **Landing embeds compaction demo** | Best proof asset is buried in `examples/` | P0 |
| **Production billing** | Real Stripe, not placeholder link; self-serve receipt | P1 |
| **Trust page** | Security, data handling, “we don’t train on your goals” | P1 |
| **`keel check` in docs + template** | CI story is the Team upsell | P1 |
| **Codex trust UX** | Users must `/hooks` trust once — easy to miss | P2 |
| **Windows** | No npm platform package yet | P2 |

### B. Go-to-market (must-have for 1%)

| Gap | Action |
|-----|--------|
| **No public launch** | Show HN + one long-form post with GIF demo |
| **No case studies** | 3 pilot teams (free Team 3 months) → quotes + logos |
| **No single wedge message** | Lead: *“Goal survives compaction; CI enforces it”* — not “agent state platform” |
| **No funnel metrics** | Track: npm installs → `keel init` → goal set → cloud link → paid |
| **No content** | One playbook: “Gate PRs on agent acceptance criteria” |
| **Founder dogfood** | This repo’s `.keel` should be a real shipping goal |

### C. Distribution (how you actually reach 1%)

| Channel | Fit |
|---------|-----|
| **npm** (`@keel2026/cli`) | Primary; matches Codex install mental model |
| **GitHub Action / template** | `keel check` on PR → viral in eng teams |
| **Claude Code / Codex communities** | Discord, X, r/ClaudeAI — compaction pain posts |
| **Dev influencers** | 30s GIF demo, not feature matrix |
| **NOT yet** | Paid ads, enterprise sales, MCP server (different product) |

### D. Organizational / ops

| Gap | Notes |
|-----|-------|
| Analytics | npm download counts only; no product telemetry (privacy-positive but blind) |
| Support | No docs site beyond README; no Discord |
| Legal | Apache-2.0 OSS; cloud ToS / privacy policy thin or missing |
| On-call | Single Render instance + SQLite — fine for early, not for “team control plane” SLA |

---

## 8. Recommended roadmap

### 0–30 days (credibility)

- [ ] Ship uncommitted `keel check` + CI example in a release
- [ ] Put compaction GIF on https://keel-cloud.onrender.com and README
- [ ] Show HN: “Repo goal survives Claude /compact”
- [ ] Fix `keel doctor` for partial `.keel` and “hooks but no goal”
- [ ] Dogfood: real goal in `compo1/.keel`
- [ ] 10 outbound DMs to eng leads using Claude Code on 3+ repos

### 31–60 days (conversion)

- [ ] 3 pilot teams with written quotes
- [ ] Stripe live + upgrade flow tested end-to-end
- [ ] Security / privacy one-pager linked from pricing
- [ ] `keel onboard` (init + interactive goal + optional cloud link)
- [ ] npm weekly install tracking + simple landing analytics

### 61–90 days (1% path)

- [ ] Cursor hook spike or documented manual hook path
- [ ] GitHub Action published (`keel-agent/check-action` or official example)
- [ ] 2k+ cumulative inits (proxy: npm installs × estimated init rate)
- [ ] 50+ paying Team seats OR clear pivot signal

---

## 9. Sales narrative (use verbatim)

**One sentence:**

> Keel Team is the control plane for AI agents in your repos — see every goal, gate merges with `keel check`, same guardrails in Claude and Codex.

**Buyer:** Eng lead / 3–15 dev shop, multiple repos, Claude Code or Codex.

**Pain:** Agents forget goals after compaction, repeat failed commands, install deps, no cross-repo visibility.

**Proof:** `examples/keel-compact-demo/demo.gif` — same task, force `/compact`, port 8842 vs 3000.

**Upgrade moment:** Second repo linked, or manager asks “which project is stuck?”

---

## 10. Operations cheat sheet

```bash
# Dev
cargo test
./scripts/stage-npm.sh
./scripts/release.sh --install-global

# Release
git tag v0.3.2 && git push origin v0.3.2

# Cloud server locally
cargo run --release --bin keel-server
# Env: PORT, KEEL_DB_PATH, KEEL_STRIPE_PAYMENT_LINK, KEEL_UPGRADE_CODES

# User install
npm install -g @keel2026/cli
cd your-repo && keel init
keel goal set "..." --accept "tests pass"
```

**Hooks:**

- Claude: `.claude/settings.json`
- Codex: `.codex/hooks.json` (trust via `/hooks`)
- Override binary: `KEEL_BIN`

**Commit guidance:** Commit `state.json` + `snapshot.md` for team goals; optional-gitignore `attempts.jsonl`, `changelog.jsonl`.

---

## 11. Experiments & evidence

### Compaction A/B (clean run, 2026-06-22)

- Global `~/.claude` Keel hooks **removed**
- **without-keel:** no `.keel`, no `.claude` hooks → PORT **3000** after `/compact`
- **with-keel:** `keel init` + goal → PORT **8842** after `/compact`

See `examples/keel-compact-demo/RESULTS.md`.

### Failed approaches (do not retry)

- `npm test` as acceptance gate in this repo without fixing tests first
- “Without Keel” tests while global Keel hooks still enabled — conflates hooks vs goal

---

## 12. Open decisions for next owner

1. **Cursor:** build first-class hooks vs stay Claude+Codex only?
2. **Telemetry:** opt-in `keel doctor --anon-stats` vs stay blind?
3. **OSS vs cloud:** keep cloud proprietary on top of Apache CLI?
4. **Pricing:** $15/mo Team — raise at 50 repos or add seat-based?
5. **Tasks API:** position as complement forever, or integrate (read task list into snapshot)?

---

## 13. Verdict for the next owner

**Technically:** v0.3 delivers the core loop. The compaction demo is real. Ship `keel check` and the GIF before anything else.

**Commercially:** You are not blocked by engineering for first 100 paying teams. You are blocked by **distribution, proof, and onboarding clarity** (`.keel` + `.claude` + goal in one story).

**1% is achievable** only as **thousands of repo inits**, not as hype — npm + Show HN + CI template + 3 case studies. Without Cursor and without a public launch, ceiling is far below 1%.

**Do not build** a full task graph, MCP server, or enterprise SSO before 10 teams pay $15/mo.

---

## 14. Key links

| Resource | URL / path |
|----------|------------|
| Cloud | https://keel-cloud.onrender.com |
| Pricing | https://keel-cloud.onrender.com/pricing |
| npm | `@keel2026/cli` |
| Compaction demo | `examples/keel-compact-demo/demo.gif` |
| CI example | `examples/github-keel-check.yml` |
| Global hooks backup | `~/.claude/settings.json.bak.before-keel-removal-*` (if removed) |

---

*Handoff prepared from full repo read, live compaction experiment, and v0.3.1 codebase state.*
