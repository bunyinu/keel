# Keel v0.2

**Repo-local agent state for Claude Code and Codex — written in Rust.**

Keel keeps task context in your repository — not in a chat transcript — so agents survive compaction, session resets, and switching between tools.

## Why Rust (v0.2)

| Python v0.1 | Rust v0.2 |
|-------------|-----------|
| Requires Python runtime | **Single static binary** (~2MB) |
| ~50ms hook cold start | **Sub-ms hook latency** |
| Basic failure detection | **Exit code + stderr parsing** |
| pip install | `cargo install` / GitHub releases |

Hooks fire on every tool call. Speed matters.

## Keel Cloud (hosted)

**Live:** https://keel-cloud.onrender.com

### As a normal user (no build tools)

1. Open **https://keel-cloud.onrender.com**
2. Enter a project name → **Create project**
3. Copy your **API key** (shown once)
4. In your repo:

```bash
npm install -g @keel-agent/cli
keel cloud link --url https://keel-cloud.onrender.com --project YOUR_PROJECT_ID --key YOUR_API_KEY
keel init
keel goal set "My task" --accept "tests pass"
```

5. Use Claude Code or Codex — state syncs to the cloud automatically.

View your snapshot anytime at `https://keel-cloud.onrender.com/dashboard/YOUR_PROJECT_ID`

### Cloud CLI commands

```bash
keel cloud link --url URL --project ID --key KEY
keel cloud push    # upload local .keel/ to cloud
keel cloud pull    # download from cloud
```

## Install (local CLI)

### npm (recommended — same pattern as Codex)

```bash
npm install -g @keel-agent/cli
cd your-repo
keel init
```

Local development from this repo:

```bash
./scripts/release.sh --install-global
keel init
```

### cargo

```bash
cargo install --path .
keel init
```

## Develop

```bash
cargo test
./scripts/stage-npm.sh          # copy release binary into npm packages
./scripts/release.sh            # fmt + test + clippy + stage + verify shim
```

## CI / Release

- **CI** (`.github/workflows/ci.yml`) — fmt, clippy, test, release build, npm shim verify
- **Release** (`.github/workflows/release.yml`) — on tag `v*.*.*`, builds 4 targets, GitHub release artifacts, npm publish (needs `NPM_TOKEN` secret)

```bash
git tag v0.2.0 && git push origin v0.2.0
```

## What it does

1. **Survives compaction** — `PreCompact` saves → `SessionStart`/`PostCompact` restores `.keel/snapshot.md`
2. **Cross-tool** — same `.keel/` for Claude Code and Codex
3. **Loop breaker** — blocks repeated failed Bash/edit (configurable threshold)
4. **v0.2 failure detection** — reads `exit_code`, `stderr`, `tool_result.is_error`

## Commands

```bash
keel init
keel goal set "..." --accept "..." --constraint "..." --step "..."
keel goal show
keel progress --step "..." --done "..." --blocker "..."
keel decide "We chose Postgres"
keel status
keel snapshot --print
```

## Layout

```
.keel/
  state.json        # goal, progress, decisions
  snapshot.md       # injected into agent context
  attempts.jsonl    # tool attempts (loop breaker)
  changelog.jsonl   # lifecycle audit log
  config.json       # thresholds
```

## Test

```bash
cargo test          # 13 tests (unit + integration)
```

## Environment

| Variable | Purpose |
|----------|---------|
| `KEEL_BIN` | Override keel binary path in installed hooks (default: `current_exe`) |

## What if Claude/Codex fix memory?

They will improve in-session memory. Keel's moat is **repo-native, cross-tool, git-versioned state** — not beating Anthropic/OpenAI at summarization.

See [vendor risk section](#what-if-claudecodex-fix-memory) in full docs below.

---

## Hooks

**Claude Code** — `.claude/settings.json`  
**Codex** — `.codex/hooks.json` (trust via `/hooks`)

After `keel init`, Codex users must review and trust hooks once.

## Commit `.keel/`?

Commit `state.json` and `snapshot.md` for team-shared task state. Optional-gitignore `attempts.jsonl` and `changelog.jsonl`.

## License

Apache-2.0
