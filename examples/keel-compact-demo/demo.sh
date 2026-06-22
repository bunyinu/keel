#!/usr/bin/env bash
set -euo pipefail

export KEEL=/home/lus/.local/bin/keel
export BASE=/tmp/keel-compact-demo-v2
export RESULTS="$BASE/results"
export TERM=${TERM:-xterm-256color}

clear 2>/dev/null || true
echo "============================================================"
echo "  KEEL vs NO KEEL — Compaction comparison (clean run)"
echo "  Global ~/.claude hooks: REMOVED"
echo "  Keel installed ONLY in with-keel/"
echo "============================================================"
echo ""
sleep 1

# --- Setup ---
rm -rf "$BASE/with-keel" "$BASE/without-keel" "$RESULTS"
mkdir -p "$RESULTS"

setup_project() {
  local dir="$1"
  mkdir -p "$dir"
  cd "$dir"
  git init -q
  git config user.email "demo@keel.test"
  git config user.name "Keel Demo"
  cat > package.json <<'EOF'
{"name":"greet-api","version":"1.0.0","type":"module","scripts":{"start":"node server.js","test":"node test.js"}}
EOF
  cat > README.md <<'EOF'
# Greet API
Minimal greeting HTTP server. No port documented here.
EOF
  git add -A && git commit -q -m "init"
}

echo ">>> Setting up twin projects..."
setup_project "$BASE/without-keel"
setup_project "$BASE/with-keel"

echo ""
echo ">>> WITHOUT KEEL: plain git repo (no keel init, no hooks)"
ls -la "$BASE/without-keel"
test ! -d "$BASE/without-keel/.keel" && echo "  ✓ no .keel"
test ! -d "$BASE/without-keel/.claude" && echo "  ✓ no .claude"

echo ""
echo ">>> WITH KEEL: keel init + goal (port 8842 is the secret requirement)"
cd "$BASE/with-keel"
$KEEL init
$KEEL goal set "Build greeting API on secret port 8842" \
  --accept "server listens ONLY on port 8842" \
  --accept 'curl http://localhost:8842/ returns {"greeting":"hello"}' \
  --constraint "never use port 3000 or 8080" \
  --step "implement server.js"
$KEEL decide "Port 8842 chosen — do not change to 3000 or 8080"
echo ""
echo "--- Keel snapshot (injected after compact) ---"
cat .keel/snapshot.md
echo ""
sleep 2

run_arm() {
  local name="$1"
  local dir="$2"
  local out="$RESULTS/$name"
  mkdir -p "$out"
  cd "$dir"

  echo ""
  echo "============================================================"
  echo "  ARM: $name"
  echo "  Directory: $dir"
  echo "============================================================"

  echo ""
  echo "--- Phase 1: Claude implements server.js ---"
  claude -p \
    --permission-mode bypassPermissions \
    --allowedTools "Read,Write,Edit,Bash" \
    --output-format json \
    "Implement server.js for a minimal greeting HTTP API. Read CLAUDE.md and .keel/snapshot.md if they exist. Use the correct port from project requirements. Write server.js and test.js. Do not start the server. Be brief in your reply." \
    > "$out/phase1.json" 2>"$out/phase1.stderr"

  SESSION=$(python3 -c "import json; print(json.load(open('$out/phase1.json'))['session_id'])")
  echo "session_id=$SESSION"
  python3 -c "import json; r=json.load(open('$out/phase1.json')).get('result',''); print(r[:800])"
  echo ""
  if [[ -f server.js ]]; then
    echo "--- server.js after phase 1 ---"
    rg -n "PORT|listen|8842|3000|8080" server.js || true
    cp server.js "$out/server_after_phase1.js"
  fi
  sleep 1

  echo ""
  echo "--- Phase 2: FORCE /compact ---"
  claude -p \
    --resume "$SESSION" \
    --permission-mode bypassPermissions \
    --output-format json \
    "/compact preserve the exact API port number, constraints, and acceptance criteria" \
    > "$out/phase2.json" 2>"$out/phase2.stderr"
  echo "Compact triggered (check transcript for compact_boundary)."
  if [[ -f .keel/snapshot.md ]]; then
    echo "Keel compactions: $(python3 -c "import json; print(json.load(open('.keel/state.json'))['compactions'])")"
  else
    echo "No .keel directory in this project."
  fi
  sleep 1

  echo ""
  echo "--- Phase 3: RECALL TEST (without re-reading files first) ---"
  claude -p \
    --resume "$SESSION" \
    --permission-mode bypassPermissions \
    --allowedTools "Read,Write,Edit,Bash" \
    --output-format json \
    "We just compacted. WITHOUT reading any files first: Line 1: PORT=<number>. Line 2: acceptance criteria you remember. Line 3: port constraints. Then read server.js and fix port if wrong." \
    > "$out/phase3.json" 2>"$out/phase3.stderr"

  python3 -c "import json; print(json.load(open('$out/phase3.json')).get('result',''))" | tee "$out/phase3.txt"
  if [[ -f server.js ]]; then
    cp server.js "$out/server_final.js"
    echo ""
    echo "--- server.js FINAL ---"
    rg -n "PORT|listen|8842|3000|8080" server.js || true
  fi
  sleep 2
}

run_arm "without-keel" "$BASE/without-keel"
run_arm "with-keel" "$BASE/with-keel"

echo ""
echo "============================================================"
echo "  FINAL COMPARISON"
echo "============================================================"
for arm in without-keel with-keel; do
  echo ""
  echo "### $arm ###"
  echo "Recall:"
  head -5 "$RESULTS/$arm/phase3.txt" 2>/dev/null || echo "(none)"
  echo "Port in server.js:"
  rg "PORT|8842|3000" "$RESULTS/$arm/server_final.js" 2>/dev/null || echo "(no file)"
  echo ".keel exists:"
  ls -d "$BASE/$arm/.keel" 2>/dev/null || echo "  NO"
done

echo ""
echo "============================================================"
echo "  DONE — results in $RESULTS"
echo "============================================================"
echo ""
echo "Recording complete."
