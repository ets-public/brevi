#!/usr/bin/env node
// Runs the prebuilt brevi binary for this platform (lib/binary.js fetches it on first use).
const { spawnSync } = require("child_process");
const { binary, InstallError } = require("../lib/binary.js");
let bin;
try {
  bin = binary();
} catch (e) {
  if (!(e instanceof InstallError)) throw e;
  console.log(JSON.stringify({ e: "install", msg: e.message, fix: e.fix }));
  process.exit(1);
}
const r = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
process.exit(r.status === null ? 1 : r.status);
