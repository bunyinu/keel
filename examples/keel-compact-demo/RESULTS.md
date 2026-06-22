# Keel vs No Keel — Compaction Demo (clean run)

**Recorded:** 2026-06-22  
**Global `~/.claude/settings.json` hooks:** removed  
**Keel:** installed only in `with-keel/` via `keel init`

## Watch the recording

- **GIF:** [demo.gif](./demo.gif) (~1 MB, terminal screen recording)
- **Asciinema cast:** [demo.cast](./demo.cast) — replay with `asciinema play demo.cast`

## What the demo does

1. Creates two identical `greet-api` repos
2. **without-keel:** plain git repo (no `.keel`, no `.claude` hooks)
3. **with-keel:** `keel init` + goal (secret port **8842**, constraints, acceptance)
4. For each: Claude Code implements `server.js` → **`/compact`** → recall test without re-reading files

## Results

| | Without Keel | With Keel |
|--|--------------|-----------|
| `.keel/` exists | **No** | Yes |
| Port after compact | **3000** (default/guess) | **8842** |
| Recalls constraints | No — "no requirement source exists" | Yes — avoids 3000/8080 |
| `server.js` | `PORT = 3000` | `PORT = 8842` |

### Without Keel (phase 3)
> 3000 is the default I chose, not a recovered requirement. The "correct port from project requirements" remains unknown, since no requirement source exists in this directory.

### With Keel (phase 3)
> `server.js` already uses `PORT = 8842` … matches the acceptance criteria and avoids the forbidden ports.

## Restore global hooks (if needed)

Backup saved at `~/.claude/settings.json.bak.before-keel-removal-*`

## Re-run

```bash
bash examples/keel-compact-demo/demo.sh
```
