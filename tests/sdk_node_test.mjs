// The Node SDK (packaging/npm), against the built binary.
//   BREVI=./target/release/brevi node tests/sdk_node_test.mjs
import path from "path";
import { fileURLToPath } from "url";
import { createRequire } from "module";
const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
process.env.BREVI_BIN = path.resolve(process.env.BREVI || path.join(root, "target/release/brevi"));
const brevi = await import(path.join(root, "packaging/npm/index.mjs"));
let fails = 0;
const expect = (label, ok, got) => {
  if (!ok) {
    fails++;
    console.log(`FAIL ${label}: ${JSON.stringify(got)}`);
  }
};

let r = await brevi.run("print(sum(range(10)))");
expect("readme run", r.stdout === "45\n" && r.exitCode === 0 && r.path === "typed" && r.ok, r);
r = await brevi.run("import sys\nprint('out')\nprint('err', file=sys.stderr)\nsys.exit(3)\n");
expect("stdout and stderr apart", r.stdout === "out\n" && r.stderr === "err\n" && r.exitCode === 3 && !r.ok, r);
r = await brevi.run("a, b = map(int, input().split())\nprint(a * b)\n", { stdin: "3 4\n" });
expect("stdin", r.stdout === "12\n", r);
r = await brevi.run("import sys\nprint(sys.argv[1:])\n", { args: ["x", "y"] });
expect("args", r.stdout === "['x', 'y']\n", r);
r = await brevi.run("with open('/etc/hostname') as f:\n    print(f.read())\n");
expect("sandbox refuses a read", !r.ok && r.text.includes("needs allow read"), r.text);
r = await brevi.run("while True:\n    pass\n", { limits: { time: 1 } });
expect("time limit", r.limit === "time" && !r.ok, [r.limit, r.text]);
r = await brevi.run("print(int(input()) * 2)\n", { cases: [{ stdin: "1\n", expect: "2" }, { stdin: "5\n", expect: "10" }] });
expect("cases", r.ok && r.text.includes("passed 2/2"), r.text);
r = await brevi.verify("print(int(input()) + 1)\n", { inputs: ["1\n", "2\n"] });
expect("verify", r.ok && r.certificate?.verify === "identical", r.text);
r = await brevi.check("print(totl)\n");
expect("check error", !r.ok && r.text.includes('"line":1'), r.text);
expect("tool has no grant field", Object.keys(brevi.TOOL.input_schema.properties).join() === "code,stdin", brevi.TOOL);
const reply = await brevi.handle({ code: "print(input()[::-1])", stdin: "abc\n" });
expect("handle", reply.startsWith("exit code 0") && reply.includes("cba"), reply);
try {
  await brevi.run("print(1)", { sandbox: false, allow: ["read"] });
  expect("grant without sandbox is an error", false, "no error");
} catch (e) {
  expect("grant without sandbox is a TypeError", e instanceof TypeError, String(e));
}
// Calls on one session at once are taken in turn.
const s = new brevi.Session();
const rs = await Promise.all([0, 1, 2].map((i) => s.run(`print(${i} * ${i})`)));
expect("parallel calls", rs.map((x) => x.stdout).join("") === "0\n1\n4\n", rs);
s.close();
// require() works too.
const req = createRequire(import.meta.url)(path.join(root, "packaging/npm/lib/sdk.js"));
expect("require", typeof req.run === "function" && req.TOOL.name === "run_python", Object.keys(req));

if (fails) {
  console.log(`${fails} Node SDK check(s) failed`);
  process.exit(1);
}
console.log("sdk node: all checks passed");
// The default session lets the process end on its own (no process.exit needed).
