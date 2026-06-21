#!/usr/bin/env node
const { spawnSync } = require("node:child_process");
const path = require("node:path");

const shim = path.join(__dirname, "..", "bin", "keel.js");
const result = spawnSync(process.execPath, [shim, "--version"], {
  encoding: "utf8",
  env: { ...process.env, KEEL_BIN: process.env.KEEL_BIN || undefined },
});

if (result.status !== 0) {
  console.error(result.stderr || result.stdout);
  process.exit(result.status || 1);
}

if (!result.stdout.includes("keel 0.2")) {
  console.error("unexpected version output:", result.stdout);
  process.exit(1);
}

console.log("shim ok:", result.stdout.trim());
