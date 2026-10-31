#!/usr/bin/env node
// Runs the prebuilt brevi binary for this platform, fetching it from the GitHub release on first use.
// The binary is kept in ~/.cache/brevi/VERSION/, so later runs start at once.
const { spawnSync, execFileSync } = require("child_process");
const fs = require("fs"), os = require("os"), path = require("path");
const version = require("../package.json").version;
const targets = {
  "linux-x64": "x86_64-unknown-linux-gnu", "linux-arm64": "aarch64-unknown-linux-gnu",
  "darwin-arm64": "aarch64-apple-darwin", "darwin-x64": "x86_64-apple-darwin", "win32-x64": "x86_64-pc-windows-msvc",
};
const target = targets[`${process.platform}-${process.arch}`];
if (!target) {
  console.log(JSON.stringify({ e: "install", msg: `no prebuilt brevi for ${process.platform}-${process.arch}`, fix: "cargo install --git https://github.com/ets-public/brevi" }));
  process.exit(1);
}
const exe = process.platform === "win32" ? "brevi.exe" : "brevi";
const dir = path.join(os.homedir(), ".cache", "brevi", version);
const bin = path.join(dir, `brevi-${target}`, exe);
if (!fs.existsSync(bin)) {
  fs.mkdirSync(dir, { recursive: true });
  const ext = process.platform === "win32" ? "zip" : "tar.gz";
  const url = `https://github.com/ets-public/brevi/releases/download/v${version}/brevi-${target}.${ext}`;
  const archive = path.join(dir, `brevi.${ext}`);
  try {
    execFileSync("curl", ["-fsSL", "-o", archive, url], { stdio: "inherit" });
    execFileSync("tar", ["-xf", archive, "-C", dir], { stdio: "inherit" });
  } catch (e) {
    console.log(JSON.stringify({ e: "install", msg: `could not fetch ${url}`, fix: "check the network, or cargo install --git https://github.com/ets-public/brevi" }));
    process.exit(1);
  }
}
const r = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
process.exit(r.status === null ? 1 : r.status);
