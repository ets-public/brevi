# Threat model: the sandbox and resource limits

This page says what `brevi run --sandbox` (and the `brevi mcp` tools, which sandbox by default) protects against, how, and what it does not cover. It is written for people deciding whether to run code an AI agent wrote through Brevi. It describes `main`; the user docs are [Sandbox](https://brevi-lang.org/docs/usage#sandbox) and [Resource limits](https://brevi-lang.org/docs/usage#resource-limits). No outside security review has been done yet.

## The setting

- **The caller** is the person or agent harness that runs `brevi`. It is trusted. It chooses the grants (`--allow`, `--allow-net`, `--allow-py`) and the limits (`--limit`).
- **The program** is untrusted Python (or Brevi) source, typically written by a model. It may be careless or hostile, and it may try to free itself.
- **The machine** is whatever runs `brevi`: the caller's laptop, a CI runner or a container. The program runs as the same OS user as `brevi`.

## What the sandbox promises

Under `--sandbox` a program gets stdin, stdout, stderr, its arguments, the clock and its own computation. Everything else is a capability the caller grants:

| Capability | Grant | Without it |
| --- | --- | --- |
| read files | `--allow read` (all), `--allow-read DIR,...` (inside those folders) | refused at build time; outside the folders, refused when opened |
| write files | `--allow write` (all), `--allow-write DIR,...` (inside those folders) | refused at build time; outside the folders, refused when opened |
| network | `--allow-net HOST,...` (named hosts) or `--allow net` (all) | refused at build time; with named hosts, a connection to any other host fails when it is made |
| CPython for some modules | `--allow-py MOD,...` | the CPython bridge is off |
| native code (`rust fn`, crates, any CPython library) | `--allow native` | refused at build time |

Under the sandbox:

1. **Only the caller grants.** An `allow` line in the program grants nothing (since v0.24.6).
2. **A refusal is final.** The capability check runs before every build path. A refused program is never built over dynamic values, never run through a library call and never run with `python3`. A sandboxed program never falls back to CPython for any reason.
3. **The environment is not read.** Neither the program nor Brevi's own libraries (argparse, httpx proxies) read environment variables.
4. **The dynamic runtime reaches nothing.** The runtime under dynamic builds has no file, process, network or CPython access of its own; the network sits in a separate layer pasted in only for a program granted it. Unit tests (`caps::tests::dynamic_runtime_reaches_nothing`, `sandbox_modules_reach_nothing`) fail the build if that changes.

## What the limits promise

`--limit cpu=S,mem=MB,time=S,output=KB,file=MB,written=MB,net=MB,threads=N,fds=N,files=N` (MCP: `limits`) bounds how much the program may use. Each limit but `files` and `net` holds with or without the sandbox:

- `cpu`, `mem` and `file` are kernel limits (`RLIMIT_CPU`, `RLIMIT_AS`, `RLIMIT_FSIZE`) set in the program's process before its first instruction on Linux and macOS. The program cannot raise them, and CPython loaded through `--allow-py` is held by them too. On Windows (from the next release) `cpu` and `mem` are a Job Object's limits: the program is created suspended, put in the job, and only then started, so its first instruction already runs inside it, and every process it starts is in the same job. `cpu` there is user-mode CPU time (Windows' job limit has no kernel-time counterpart) and `mem` is committed memory of the whole job.
- `fds` is a kernel limit too (`RLIMIT_NOFILE`): an open past it fails inside the program.
- `time` and `output` are held by the `brevi` process that started the program, which stops it (SIGKILL) once either is reached.
- `threads` and `written` (Linux) are the kernel's own counts for the program's process (`/proc/PID/status` threads, `/proc/PID/io` bytes written less what went to stdout and stderr), read by `brevi` every few milliseconds; the program can overshoot them by what it does between two reads. Processes the program starts (only possible with `--allow native` or `--allow-py`) are not counted.
- `files` and `net` are counted by Brevi's own file and network code inside the program, so they need `--sandbox` without `--allow-py` or `--allow native` (refused otherwise): a program with neither has no other way to create a file or open a socket.
- A limit the platform cannot enforce is refused, never silently dropped: `mem` on macOS; `written` and `threads` outside Linux; `file` and `fds` on Windows.
- A program stopped by a limit exits 124 and its last stderr line names the limit.

## How it is enforced, and where that is weaker than a VM

The sandbox is **a compile-time capability check plus a runtime with nothing in it**, not an operating-system jail. There is no seccomp filter, namespace or VM around the program. The one kernel layer is for CPython: on Linux, a program that loads CPython under `--sandbox` is held to its file grants by Landlock (below). That is what makes it cheap (no boot time, native speed), and it means:

- **The guarantee rests on Brevi being right.** A bug that lets source reach a system call Brevi did not mean to expose (a missed name in the capability check, a runtime function that touches the file system) is an escape. So is any memory-safety bug in generated code or the runtime that a program can steer into running its own machine code. Both are treated as security bugs ([SECURITY.md](../SECURITY.md)).
- **`--allow-py` and `--allow native` are full trust.** Once CPython is loaded, Python code can reach `os` through object introspection whatever module was named, and `rust fn` or a crate is arbitrary native code. Grant them only to code you would run unsandboxed. `--allow-py` limits which modules a program may import; it does not contain what those modules do, with one exception: files. On Linux (5.13 or later, with Landlock on), before CPython starts, the kernel holds the whole program to its file grants with Landlock, which needs no privileges and cannot be undone. CPython, its C extensions and native code can then read only the granted folders plus CPython's own files (its home, `sys.path`, shared libraries, the libraries granted from the program's folder) and a short list of system files (TLS certificates, DNS and time zone files, `/dev/urandom`), and can write, make, rename or remove files only in the folders granted write. With no read or write grant, that means no files at all. Where Landlock is missing, a caller who named folders with `--allow-read` / `--allow-write` is refused rather than given every folder; with no folders named, CPython keeps full file access there. The network, processes and the `files` and `net` limits are not held for CPython (the kernel limits are).
- **`--allow read` and `--allow write` are whole-account grants**: a program with `--allow write` can write anywhere the user can. `--allow-read DIR` and `--allow-write DIR` hold it to folders instead. Brevi resolves each path the program opens (`..` and symlinks) and opens the resolved path only if it lies inside a granted folder. The check and the open are two steps, so another process that swaps a folder for a symlink between them could redirect the open; the program itself has no way to make a symlink. Under `--allow-py` or `--allow native` the kernel holds CPython to the same folders on Linux (see above).
- **The network grant matches host names, not addresses.** `--allow-net api.example.com` lets the program reach whatever that name resolves to. Under `--allow net` the program can reach anything the machine can, including services on `localhost` and cloud metadata endpoints.

For hostile code, the sandbox and limits are a first layer. Running `brevi` itself inside a container or VM with no credentials is still the advice for code you have reason to distrust.

## What is not covered

- **Output is untrusted.** Whatever the program prints reaches the caller, and an agent reading it can be steered by it (prompt injection). The `output` limit bounds its size, not its content.
- **Side channels.** Timing, CPU load and memory use are visible to the program and to anything else on the machine.
- **Resources without a limit:** the number of connections to granted hosts (`net` bounds their bytes, `fds` how many are open at once), disk space a program uses by writing over the same bytes (`written` counts bytes written, not space used), and writes through memory maps (which only CPython or native code can make).
- **Output the program had buffered when it was stopped** is lost; output it wrote before is kept.
- **The build.** Limits hold the program, not `brevi build` or `rustc`. A build is bounded by the MCP call's `timeout` (or by the caller). Brevi compiles source to Rust and runs `rustc` on it; the program cannot add Rust of its own without `native`.
- **Who reaches `brevi mcp --http`.** Anyone who can send it a request can run programs under the tools' sandbox, so on an address other than 127.0.0.1 it refuses to start without a bearer token (`--token` or `BREVI_MCP_TOKEN`, at least 16 characters) and refuses requests from web origins that are not local unless `--allow-origin` names them. It speaks plain HTTP: keep the token on a trusted network or behind the platform's TLS port forwarding, since anyone who reads the token can use the server.
- **The machine's own state:** a malicious `rustc`, Python or shared library on `PATH`, or a build cache written by another user, is outside this model.

## Reporting

A sandboxed program that reads or writes files, reaches the network or the environment, or runs native code it was not granted, or a limit that does not hold where this page says it does, is a vulnerability: report it privately as [SECURITY.md](../SECURITY.md) describes.
