# Keel v0.3

**Repo-local agent state for Claude Code and Codex — written in Rust.**

Keel keeps task context in your repository — not in a chat transcript — so agents survive compaction, session resets, and switching between tools.

## Install & update (everyone)

**One standard method** — npm, same pattern as Codex CLI. Requires [Node.js 18+](https://nodejs.org/).

```bash
# Install
npm install -g @keel2026/cli

# Update (after you have 0.2.2+)
keel update

# Or update without the keel command
npm install -g @keel2026/cli@latest
```

Verify:

```bash
keel --version
```

Then in your project:

```bash
cd your-repo
keel init
keel goal set "My task" --accept "tests pass"
# or: keel tui
```

> **Contributors** building from this repo use `./scripts/release.sh --install-global` — not required for normal users.

## Keel Cloud (hosted)

**Live:** https://keel-cloud.onrender.com · **Pricing:** https://keel-cloud.onrender.com/pricing

| Plan | Price | What you sell |
|------|-------|----------------|
| **Free** | $0 | 1 repo on cloud + full local CLI |
| **Team** | $15/mo | Fleet dashboard + 50 repos + `keel check` in CI |

Pro activation: pay via Stripe on `/pricing`, then redeem upgrade code with your team license.

### What you’re selling (one sentence)

> **Keel Team is the control plane for AI agents in your repos** — see every goal, gate merges with `keel check`, same guardrails in Claude and Codex.

**Buyer:** eng lead / small team (3–15 devs) using Claude Code or Codex on multiple repos.  
**Pain:** agents forget goals after compaction, skip tests, install deps, no visibility across repos.  
**Proof:** `keel check` fails CI until goal + tests pass; fleet dashboard shows which repo is stuck.

Example CI workflow: [`examples/github-keel-check.yml`](examples/github-keel-check.yml)

1. Open **https://keel-cloud.onrender.com** → create a project → copy your API key
2. In your repo:

```bash
keel cloud link --url https://keel-cloud.onrender.com --project YOUR_PROJECT_ID --key YOUR_API_KEY
keel init
```

3. Use Claude Code or Codex — state syncs automatically.

**Dashboard:** `https://keel-cloud.onrender.com/dashboard/YOUR_PROJECT_ID`  
**Edit goal in browser:** `.../dashboard/YOUR_PROJECT_ID/edit`

## Set your goal (CLI, TUI, or web)

| Method | Command / URL |
|--------|----------------|
| CLI | `keel goal set "..." --accept "..." --constraint "..." --step "..."` |
| TUI | `keel tui` |
| Web | `https://keel-cloud.onrender.com/dashboard/YOUR_PROJECT_ID/edit` |

After editing on the web: `keel cloud pull` in your repo.

## Commands

```bash
keel init
keel onboard "My task" --accept "tests pass"   # recommended: init + goal
keel doctor                       # diagnose setup
keel check                        # CI: goal + acceptance gate (if enabled)
keel check --cloud                # also verify cloud link
keel update                       # npm users: upgrade to latest
keel goal set / show
keel tui
keel config set --acceptance "npm test"   # gate on agent stop
keel config set --acceptance off
keel config show
keel progress --step "..." --done "..." --blocker "..."
keel decide "We chose Postgres"
keel status
keel snapshot --print
keel cloud link / push / pull
```

## v0.3 guardrails

| Feature | When | What |
|---------|------|------|
| **Constraint guard** | `PreToolUse` | Blocks deps install, file edits (read-only), banned keywords from `--constraint` |
| **Acceptance gate** | `Stop` | Runs your command (e.g. `npm test`) before agent ends session |
| **Loop breaker** | `PreToolUse` | Blocks repeated failed commands (unchanged) |
| **Trusted snapshot** | compaction / session start | Frames goal as user-defined requirements, not injection |

Constraints are matched from `keel goal set --constraint "..."`. Examples:

- `no new deps` → blocks `npm install`, `cargo add`, etc.
- `read-only` → blocks Write/Edit
- `no payment SDK` → blocks stripe/paypal in commands

## What it does

1. **Survives compaction** — hooks restore `.keel/snapshot.md` after compact/resume
2. **Cross-tool** — same `.keel/` for Claude Code and Codex
3. **Loop breaker** — blocks repeated failed Bash/edit attempts
4. **Failure detection** — reads `exit_code`, `stderr`, `tool_result.is_error`

## Keel vs Claude Tasks API vs Agentpack

Nobody ships *exactly* Keel as a first-party product. These are the closest alternatives today.

| | **Keel** | **Claude Code Tasks API** | **[Agentpack](https://github.com/ihorponom/agentpack)** |
|---|----------|---------------------------|----------------------------------------------------------|
| **Who** | Third-party (you) | Anthropic (native) | Third-party OSS |
| **Agents** | Claude Code + Codex + Cursor | Claude Code only | Any MCP client (Claude, Codex, Cursor, …) |
| **Where state lives** | `.keel/` **in the repo** (git-committable) | `~/.claude/tasks/` (home dir) | `.agentpack/` (local; gitignored by default) |
| **Who writes state** | You (`keel goal set`, TUI, web) + hooks | Agent (`TaskCreate` / `TaskUpdate`) | Agent via MCP tools + checkpoints |
| **Survives compaction** | ✓ hooks inject `snapshot.md` | ✓ tasks on disk, agent queries `TaskList` | ✓ ledger + `load_context` / export |
| **Multi-session sync** | ✓ git + optional Keel Cloud | ✓ `CLAUDE_CODE_TASK_LIST_ID` | ✓ shared ledger / handoff export |
| **Acceptance criteria** | ✓ explicit in goal + optional **Stop gate** | ✗ task status only | ✓ evidence / decisions (no Stop gate) |
| **Enforcement** | ✓ constraint guard, loop breaker, acceptance gate | ✗ reminders + task status (no tool blocks) | ✗ continuity layer (no tool blocks) |
| **Install** | `npm install -g @keel2026/cli` | Built into Claude Code v2.1.16+ | `pip` / MCP server |
| **Hosted team UI** | ✓ Keel Cloud (free / Pro) | ✗ | ✗ |

### When to use which

- **Claude Code Tasks API** — you live in Claude only, want native task lists with dependencies and multi-terminal sync. Best default *inside* Claude Code since v2.1.19.
- **Agentpack** — you want a rich task ledger (decisions, dead ends, evidence, file hashes) and already run MCP across multiple agents.
- **Keel** — you want **repo-owned** goal state in git, the **same file** in Claude *and* Codex, **hard guardrails** (block deps, block stop until tests pass), and optional cloud/team dashboard without running an MCP server.

Keel complements Tasks API and Agentpack; it does not replace Claude memory, `CLAUDE.md`, or a full ledger. Use Tasks or Agentpack for deep task graphs; use Keel when the team needs a shared, enforceable goal file in the repo.

## Building workflow dependency (stickiness)

Keel should not trap users — it should **accumulate value** so removing it hurts workflow, not data.

| Layer | What compounds | Switching cost |
|-------|----------------|----------------|
| **Git** | Commit `.keel/state.json` + `snapshot.md` — goal becomes part of PR review | Team process references Keel goal in tickets/PRs |
| **Hooks** | Constraint guard + loop breaker + Stop gate run every session | Agents behave differently without hooks |
| **History** | `decisions`, `attempts.jsonl`, `changelog.jsonl` — “do not retry” list grows | Lose failure memory if you uninstall |
| **Cloud** | Pro team links many repos; web goal editor | Re-link every repo + lose dashboard |
| **CI** | `keel check` in GitHub Actions / pre-merge | Pipeline fails without Keel |

**CI example** (add after `keel config set --acceptance "npm test"`):

```yaml
- name: Keel acceptance
  run: keel check
```

`keel check` verifies: Keel initialized → active goal → acceptance command passes (same gate as agent Stop). Use `keel check --cloud` when linked to Keel Cloud.

## Layout

```
.keel/
  state.json        # goal, progress, decisions
  snapshot.md       # injected into agent context
  attempts.jsonl    # tool attempts (loop breaker)
  changelog.jsonl   # lifecycle audit log
  config.json       # thresholds
  cloud.json        # optional Keel Cloud link
```

## Why Rust

| Python v0.1 | Rust v0.2 |
|-------------|-----------|
| Requires Python runtime | **Single static binary** (~2MB) |
| ~50ms hook cold start | **Sub-ms hook latency** |
| pip install | **npm install -g @keel2026/cli** |

## Develop (contributors only)

```bash
cargo test
./scripts/stage-npm.sh
./scripts/release.sh --install-global
```

## CI / Release

- **CI** — fmt, clippy, test, npm shim verify
- **Release** — tag `v*.*.*` → GitHub binaries + npm publish

```bash
git tag v0.2.2 && git push origin v0.2.2
```

## Environment

| Variable | Purpose |
|----------|---------|
| `KEEL_BIN` | Override keel binary path in installed hooks |

## Hooks

**Claude Code** — `.claude/settings.json`  
**Codex** — `.codex/hooks.json` (trust via `/hooks`)

After `keel init`, Codex users must review and trust hooks once.

## Commit `.keel/`?

Commit `state.json` and `snapshot.md` for team-shared task state. Optional-gitignore `attempts.jsonl` and `changelog.jsonl`.

## License

Apache-2.0
