#!/usr/bin/env node
/** Rewrite optionalDependencies to registry versions before npm publish. */
const fs = require("node:fs");
const path = require("node:path");

const version = process.argv[2];
if (!version) {
  console.error("usage: prep-publish.js <version>");
  process.exit(1);
}

const pkgPath = path.join(__dirname, "..", "package.json");
const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8"));

pkg.optionalDependencies = {
  "@keel-agent/linux-x64-gnu": version,
  "@keel-agent/linux-arm64-gnu": version,
  "@keel-agent/darwin-x64": version,
  "@keel-agent/darwin-arm64": version,
};

fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");
console.log("Prepared @keel-agent/cli for npm publish");
