# Keel v0.2

**Repo-local agent state for Claude Code and Codex — written in Rust.**

Keel keeps task context in your repository — not in a chat transcript — so agents survive compaction, session resets, and switching between tools.

## Install & update (everyone)

**One standard method** — npm, same pattern as Codex CLI. Requires [Node.js 18+](https://nodejs.org/).

```bash
# Install
npm install -g @keel-agent/cli

# Update (after you have 0.2.2+)
keel update

# Or update without the keel command
npm install -g @keel-agent/cli@latest
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

| Plan | Price | Projects |
|------|-------|----------|
| **Free** | $0 | 1 cloud project |
| **Pro** | $15/mo | 50 projects + team dashboard |

Pro activation: pay via Stripe link on `/pricing`, then redeem upgrade code with your team license.

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
keel doctor                       # diagnose setup
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
| pip install | **npm install -g @keel-agent/cli** |

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
