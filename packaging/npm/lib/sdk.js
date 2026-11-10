// The Brevi SDK for Node: run AI-written Python natively and sandboxed, as the Python SDK does.
//
//   import { run, verify, TOOL, handle, Session } from "brevi";
//   const r = await run("print(sum(range(10)))");    // sandboxed, nothing granted, default limits
//   r.stdout, r.exitCode, r.path                      // "45\n", 0, "typed"
//   await run(code, { stdin: "3 4\n", limits: { time: 5, mem: 256 }, allowRead: ["/data"] });
//   await verify(code, { inputs: ["1\n", "2\n"] });   // r.certificate.verify === "identical"
//
// Each Session is one `brevi mcp` process spoken to over stdio, so the build cache stays warm. Grants and
// limits come only from the calling code: TOOL gives the model no field for them.
const { spawn } = require("child_process");
const { binary, version } = require("./binary.js");

const TOOL = {
  name: "run_python",
  description:
    "Run a Python 3 program and get its output. It is compiled to native code, so write the plain, direct " +
    "solution; do not hand-optimise it for speed. It reads `stdin` and prints to stdout. Files, the network " +
    "and other processes are only available if the operator granted them; an error says what was refused.",
  input_schema: {
    type: "object",
    properties: {
      code: { type: "string", description: "The whole program (main.py)." },
      stdin: { type: "string", description: "Text passed to the program on stdin." },
    },
    required: ["code"],
    additionalProperties: false,
  },
};

const GRANTS = { allow: "allow", allowPy: "allow_py", allowNet: "allow_net", allowRead: "allow_read", allowWrite: "allow_write" };

class BreviError extends Error {
  constructor(msg) {
    super(msg);
    this.name = "BreviError";
  }
}

// What one call gave back. stdout, stderr and exitCode are the program's own (exitCode null when it did
// not run to an exit); path is how it ran ("typed", "dynamic", "pybridge", "cpython", or null when it did
// not build); limit names the limit that stopped it; certificate is verify's verdict; text is the reply
// as the MCP tool gives it to a model.
class Result {
  constructor(text, isError, s = {}) {
    s = s || {};
    this.text = text;
    this.ok = !isError;
    this.path = s.path ?? null;
    this.ms = s.ms ?? null;
    this.timedOut = !!s.timed_out;
    if ("stdout" in s) {
      this.stdout = s.stdout;
      this.stderr = s.stderr;
      this.exitCode = s.exit_code ?? null;
    } else {
      this.stdout = isError ? "" : text;
      this.stderr = isError ? text : "";
      this.exitCode = null;
    }
    this.certificate = null;
    const v = text.split("\n").reverse().find((l) => l.startsWith('{"verify":'));
    if (v) {
      try {
        this.certificate = JSON.parse(v);
      } catch {}
      if (this.certificate) this.ok = this.ok && this.certificate.verify === "identical";
    }
  }

  get limit() {
    for (const line of (this.stderr + "\n" + this.text).split("\n")) {
      if (line.startsWith('{"e":"limit"')) {
        try {
          return JSON.parse(line).on ?? null;
        } catch {
          return null;
        }
      }
    }
    return null;
  }

  // What an agent reads back: the exit code, stdout and stderr, each cut to `cap` characters.
  forModel(cap = 8000) {
    const cut = (s) => (s.length <= cap ? s : s.slice(0, cap) + `\n[... ${s.length - cap} more characters]`);
    if (this.exitCode === null) return cut(this.text);
    const parts = [`exit code ${this.exitCode}`];
    if (this.stdout) parts.push("stdout:\n" + cut(this.stdout));
    if (this.stderr) parts.push("stderr:\n" + cut(this.stderr));
    return parts.join("\n");
  }
}

// The tools/call arguments: the session's defaults, then this call's options.
function toolArgs(defaults, code, opts) {
  const o = { ...defaults, ...opts };
  const a = { sandbox: o.sandbox !== false };
  for (const [k, wire] of Object.entries(GRANTS)) {
    if (o[k] && o[k].length) {
      if (!a.sandbox) throw new TypeError(`${k} only applies under the sandbox; with sandbox: false everything is allowed`);
      a[wire] = [...o[k]];
    }
  }
  if (o.limits) a.limits = { ...o.limits };
  if (o.timeout) a.timeout = Math.trunc(o.timeout);
  if (code !== undefined) a.code = code;
  return a;
}

class Session {
  // defaults: sandbox (default true), allow, allowPy, allowNet, allowRead, allowWrite, limits, timeout,
  // cwd (where brevi mcp starts), brevi (the executable). Each call may override them.
  constructor(defaults = {}) {
    const { cwd, brevi, ...rest } = defaults;
    this.defaults = rest;
    this.cwd = cwd;
    this.brevi = brevi;
    this.proc = null;
    this.id = 0;
    this.waiting = new Map();
    this.queue = Promise.resolve();
  }

  start() {
    const exe = this.brevi || binary();
    // --trust-calls: the calls are this code's (the model only fills TOOL's code and stdin), so the grants and
    // sandbox setting they carry are the caller's own.
    const p = spawn(exe, ["mcp", "--trust-calls"], { cwd: this.cwd, stdio: ["pipe", "pipe", "ignore"] });
    this.proc = p;
    let buf = "";
    p.stdout.setEncoding("utf8");
    p.stdout.on("data", (chunk) => {
      buf += chunk;
      let i;
      while ((i = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, i);
        buf = buf.slice(i + 1);
        if (!line.trim()) continue;
        let msg;
        try {
          msg = JSON.parse(line);
        } catch {
          continue;
        }
        const w = this.waiting.get(msg.id);
        if (w) {
          this.waiting.delete(msg.id);
          w.resolve(msg);
        }
      }
    });
    const fail = (why) => {
      if (this.proc === p) this.proc = null;
      for (const w of this.waiting.values()) w.reject(new BreviError(why));
      this.waiting.clear();
    };
    p.on("error", (e) => fail(`cannot start ${exe} mcp: ${e.message}`));
    p.on("exit", () => fail("brevi mcp stopped without answering"));
    p.stdin.on("error", () => {});
    const init = this.request("initialize", {
      protocolVersion: "2025-06-18",
      capabilities: {},
      clientInfo: { name: "brevi-node-sdk", version },
    });
    return init.then(() => this.send({ jsonrpc: "2.0", method: "notifications/initialized" }));
  }

  send(msg) {
    this.proc.stdin.write(JSON.stringify(msg) + "\n");
  }

  request(method, params) {
    const id = ++this.id;
    return new Promise((resolve, reject) => {
      this.waiting.set(id, { resolve, reject });
      this.send({ jsonrpc: "2.0", id, method, params });
    });
  }

  // Calls one MCP tool (check, run, test, verify, patch, edit, show, guide); calls are taken one at a time.
  call(tool, args) {
    const next = this.queue.then(async () => {
      if (!this.proc) await this.start();
      const msg = await this.request("tools/call", { name: tool, arguments: args });
      if (msg.error) throw new BreviError(`brevi mcp: ${msg.error.message}`);
      const r = msg.result;
      const text = (r.content || []).filter((c) => c.type === "text").map((c) => c.text).join("");
      return new Result(text, !!r.isError, r.structuredContent);
    });
    this.queue = next.catch(() => {});
    return next;
  }

  // Builds `code` (plain Python) natively and runs it. opts: stdin, args, cases, and any session default.
  run(code, opts = {}) {
    const { stdin, args, cases, ...rest } = opts;
    const a = toolArgs(this.defaults, code, rest);
    if (stdin) a.stdin = stdin;
    if (args && args.length) a.args = [...args];
    if (cases && cases.length) a.cases = cases;
    return this.call("run", a);
  }

  // Builds without running: ok, or the errors as one JSON line each in .text.
  check(code, opts = {}) {
    const { fast, ...rest } = opts;
    const a = toolArgs(this.defaults, code, rest);
    if (fast) a.fast = true;
    return this.call("check", a);
  }

  // Runs the native build and CPython on each input and compares them; .certificate is the verdict.
  // It runs CPython on the code, which no sandbox holds, so only verify code you would run with python3.
  verify(code, opts = {}) {
    const a = { sandbox: false, code };
    if (opts.inputs && opts.inputs.length) a.inputs = [...opts.inputs];
    if (opts.args && opts.args.length) a.args = [...opts.args];
    if (opts.once) a.once = true;
    const limits = opts.limits || this.defaults.limits;
    if (limits) a.limits = { ...limits };
    const timeout = opts.timeout || this.defaults.timeout;
    if (timeout) a.timeout = Math.trunc(timeout);
    return this.call("verify", a);
  }

  // Runs a model's run_python tool call (TOOL) and returns the text to send back.
  async handle(input) {
    return (await this.run(input.code || "", { stdin: input.stdin || "" })).forModel();
  }

  close() {
    const p = this.proc;
    this.proc = null;
    if (p) p.stdin.end();
  }
}

let shared = null;
function session() {
  if (!shared) {
    shared = new Session();
    // The default session must not keep the process alive once nothing else does.
    const unref = () => {
      if (shared && shared.proc) {
        shared.proc.unref();
        shared.proc.stdin.unref?.();
        shared.proc.stdout.unref?.();
      }
    };
    const call = shared.call.bind(shared);
    shared.call = (...a) => {
      const p = call(...a);
      p.then(unref, unref);
      if (shared.proc) {
        shared.proc.ref();
        shared.proc.stdout.ref?.();
      }
      return p;
    };
  }
  return shared;
}

const run = (code, opts) => session().run(code, opts);
const check = (code, opts) => session().check(code, opts);
const verify = (code, opts) => session().verify(code, opts);
const handle = (input) => session().handle(input);

module.exports = { TOOL, Result, BreviError, Session, run, check, verify, handle, binary };
