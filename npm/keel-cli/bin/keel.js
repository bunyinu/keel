#!/usr/bin/env node
/**
 * Keel npm shim — resolves the native binary for the current platform.
 * Pattern follows OpenAI Codex CLI (@openai/codex).
 */
const { spawn } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const PLATFORM_PACKAGE_BY_TARGET = {
  "linux-x64": "@keel-agent/linux-x64-gnu",
  "linux-arm64": "@keel-agent/linux-arm64-gnu",
  "darwin-x64": "@keel-agent/darwin-x64",
  "darwin-arm64": "@keel-agent/darwin-arm64",
};

function platformKey() {
  return `${process.platform}-${process.arch}`;
}

function quoteIfNeeded(p) {
  return p.includes(" ") ? `"${p}"` : p;
}

function resolveFromOptionalPackage() {
  const key = platformKey();
  const pkgName = PLATFORM_PACKAGE_BY_TARGET[key];
  if (!pkgName) return null;

  try {
    const pkgJson = require.resolve(`${pkgName}/package.json`);
    const pkgDir = path.dirname(pkgJson);
    const binPath = path.join(pkgDir, "bin", "keel");
    if (fs.existsSync(binPath)) return binPath;
  } catch {
    // optional dependency not installed
  }
  return null;
}

function resolveVendorBinary() {
  const vendor = path.join(__dirname, "..", "vendor", "keel");
  if (fs.existsSync(vendor)) return vendor;
  const dev = path.join(__dirname, "..", "..", "..", "target", "release", "keel");
  if (fs.existsSync(dev)) return dev;
  return null;
}

function resolveBinary() {
  if (process.env.KEEL_BIN) return process.env.KEEL_BIN;

  const fromPkg = resolveFromOptionalPackage();
  if (fromPkg) return fromPkg;

  const vendor = resolveVendorBinary();
  if (vendor) return vendor;

  return "keel";
}

function main() {
  const bin = resolveBinary();
  const args = process.argv.slice(2);

  const child = spawn(bin, args, {
    stdio: "inherit",
    env: {
      ...process.env,
      KEEL_MANAGED_BY_NPM: "1",
    },
  });

  child.on("error", (err) => {
    if (err.code === "ENOENT") {
      console.error(
        "keel: native binary not found.\n" +
          "Install with: npm install -g @keel-agent/cli\n" +
          "Or build from source: cargo install --path ."
      );
    } else {
      console.error(`keel: ${err.message}`);
    }
    process.exit(1);
  });

  child.on("close", (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code ?? 1);
  });
}

main();
