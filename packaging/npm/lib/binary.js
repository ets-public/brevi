// The prebuilt brevi binary for this platform, fetched from the GitHub release on first use and kept in
// ~/.cache/brevi/VERSION/, so later runs start at once. BREVI_BIN names another binary instead.
const { execFileSync } = require("child_process");
const fs = require("fs"), os = require("os"), path = require("path");
const version = require("../package.json").version;
const targets = {
  "linux-x64": "x86_64-unknown-linux-gnu", "linux-arm64": "aarch64-unknown-linux-gnu",
  "darwin-arm64": "aarch64-apple-darwin", "darwin-x64": "x86_64-apple-darwin", "win32-x64": "x86_64-pc-windows-msvc",
};

class InstallError extends Error {
  constructor(msg, fix) {
    super(msg);
    this.name = "InstallError";
    this.fix = fix;
  }
}

function binary() {
  if (process.env.BREVI_BIN) return process.env.BREVI_BIN;
  const target = targets[`${process.platform}-${process.arch}`];
  if (!target) throw new InstallError(`no prebuilt brevi for ${process.platform}-${process.arch}`, "cargo install --git https://github.com/ets-public/brevi");
  const exe = process.platform === "win32" ? "brevi.exe" : "brevi";
  const dir = path.join(os.homedir(), ".cache", "brevi", version);
  const bin = path.join(dir, `brevi-${target}`, exe);
  if (!fs.existsSync(bin)) {
    fs.mkdirSync(dir, { recursive: true });
    const ext = process.platform === "win32" ? "zip" : "tar.gz";
    const url = `https://github.com/ets-public/brevi/releases/download/v${version}/brevi-${target}.${ext}`;
    const archive = path.join(dir, `brevi.${ext}`);
    try {
      // stderr only: stdout may be an MCP stream.
      execFileSync("curl", ["-fsSL", "-o", archive, url], { stdio: ["ignore", "ignore", "inherit"] });
      execFileSync("tar", ["-xf", archive, "-C", dir], { stdio: ["ignore", "ignore", "inherit"] });
    } catch (e) {
      throw new InstallError(`could not fetch ${url}`, "check the network, or cargo install --git https://github.com/ets-public/brevi");
    }
  }
  return bin;
}

module.exports = { binary, version, InstallError };
