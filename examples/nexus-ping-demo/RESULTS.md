# Nexus-Ping — Claude `/compact` with vs without Keel

**Recorded:** 2026-06-23 (fair baseline: both arms have `CLAUDE.md` + `.claude/`)  
**Project:** `nexus-ping` (fresh app; secret port **7429** only in Keel goal)  
**Agent:** Claude Code CLI (`claude -p`) — **not** a Keel simulation

## What “compaction” means here

This demo uses **Claude Code’s own `/compact`** — the same slash command you run in the agent when context is summarized.

Flow per arm (same Claude session, `--resume`):

1. **Phase 1** — Claude implements `server.js` from a vague README (no port in repo).
2. **Phase 2** — Send **`/compact`** to that session (`claude -p --resume <id> "/compact …"`).
3. **Phase 3** — Ask Claude to recall port/constraints **without reading files first**, then fix `server.js` if wrong.

**Without Keel:** normal Claude project (`CLAUDE.md`, `.claude/settings.json`) but **`keel init` not run** — no `.keel/`, no Keel hooks on PreCompact.  
**With Keel:** same project + `keel init` → `.keel/snapshot.md` injected when Claude runs `/compact`.

## Proof Claude actually compacted (with-keel arm)

From `.keel/changelog.jsonl` on the recorded run:

```json
{"event":"pre_compact","trigger":"manual"}   ← Claude /compact
{"event":"session_start","source":"compact"} ← new session after compact
```

Phase 2 JSON: `num_turns: 0` (compaction is not a normal chat turn).

## Watch

| | |
|--|--|
| GIF | [demo.gif](./demo.gif) |
| Cast | `asciinema play demo.cast` |
| Raw | [artifacts/results/](./artifacts/results/) |
| Audit log | [artifacts/with-keel-changelog.jsonl](./artifacts/with-keel-changelog.jsonl) |

## Results after Claude compaction

| | Without Keel | With Keel |
|--|--------------|-----------|
| PreCompact hook | **No Keel hooks** (plain `.claude`) | **Keel** → `systemMessage` + snapshot |
| Port in `server.js` | **`process.env.PORT`** (no value — defers to env) | **7429** (hardcoded) |
| Route / response | `GET /health` → `{status:'ok'}` | `GET /` → `{nexus:'online',ping:true}` |
| Phase 3 recall | No port number — “never hardcoded, use env” | **7429** + correct JSON shape |

### Without Keel (after `/compact`, fair run 2026-06-23)

> No fix needed. `server.js` sources the port from `process.env.PORT` and never hardcodes one … There's no wrong port to correct, since none was ever invented.

Agent followed `CLAUDE.md` (“do not invent ports”) but **cannot ship the secret port 7429** — it doesn't know it exists after compact.

### With Keel (after `/compact`, fair run 2026-06-23)

> `server.js` already uses `PORT = 7429` and returns `{"nexus":"online","ping":true}` on `GET /`. No fix needed.

## Re-run

```bash
bash examples/nexus-ping-demo/demo.sh
bash examples/nexus-ping-demo/record.sh   # asciinema + GIF
```

Keel only **survives** compaction when hooks are installed (`keel init`). The comparison is: same Claude agent, same `/compact`, different hook layer.
