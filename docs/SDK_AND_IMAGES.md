# Design: SDK, `python3` shim and base images

Status: design, 9 November 2026 (ROADMAP.md, WS5). Builds on what exists: the PyPI wheel and npm wrapper that put
the `brevi` binary on PATH (`packaging/`), the MCP server (`brevi mcp`, stdio and `--http`), and the agent platform
templates with their shared image (`templates/`).

## Goal

A harness or sandbox platform can make Brevi its Python with one dependency and no Rust knowledge, and a user can go
from nothing to a native run in 60 seconds or less without installing Rust.

## 1. The SDK

### Python: `import brevi`

The `brevi` wheel already ships the binary. It gains a small pure-Python module in the same wheel, so there is still
one thing to install:

```python
import brevi

r = brevi.run("print(sum(range(10)))")                       # sandboxed, no grants, default limits
r.stdout, r.exit_code, r.path        # '45\n', 0, 'typed'
r = brevi.run(code, stdin="3 4\n", limits={"time": 5, "mem": 256}, allow_read=["data/"])
r = brevi.run(code, cases=[{"stdin": "1\n", "expect": "2"}])  # per-case ok or first differing line
r = brevi.verify(code, inputs=["1\n", "2\n"])                 # certificate (sandbox off, see below)
r.for_model()                        # the short text to hand back to the model (as templates/brevi_tool.py does)
```

- **No shelling out per call.** The module starts one `brevi mcp` child per `brevi.Session` (a default session is made
  on first use) and sends it JSON-RPC over stdio. That process already keeps the build cache warm and gives the
  structured replies the MCP tools give, so the SDK is a thin client and the daemon of WS2 drops in behind it later.
- **The same safe defaults as the MCP server:** `sandbox=True`, no grants, the caller passes `allow`, `allow_py`,
  `allow_net`, `allow_read`, `allow_write` and `limits`. Code can never grant itself anything.
- **Result object:** `stdout`, `stderr`, `exit_code`, `path` (`typed`, `dynamic`, `pybridge`, `cpython` or `None`),
  `limit` (which limit stopped it), `certificate` (from verify), `ms`. `for_model(cap=8000)` trims for a model.
- **Async:** `brevi.AsyncSession` with the same methods, for harnesses built on asyncio.
- **Tests:** `tests/sdk_test.py` runs the README example and each argument against the built binary in CI (Linux only,
  no extra minutes beyond the existing test job).

### Node: `import { run } from "brevi"`

The npm package already downloads the binary on first run. It gains `index.mjs` with the same `run`, `verify` and
`Session` over `brevi mcp`, and TypeScript types. Second, after the Python SDK.

### Tool definition

Both SDKs export the tool schema harnesses hand to a model (`brevi.TOOL`, as `templates/brevi_tool.py` has today):
one `run_python(code, stdin)` tool with no permission fields, so the model cannot ask for grants.

## 2. The `python3` shim: `brevi-python`

A drop-in for `python3` that builds natively when it can and is CPython otherwise, so a harness can switch with one
PATH change and no prompt changes.

| Invocation | What `brevi-python` does |
|---|---|
| `brevi-python script.py [args]` | `brevi run script.py [args]`: native when it builds, CPython otherwise (no output from Brevi on stdout) |
| `brevi-python -c CODE` | the same, on a temp file |
| `brevi-python - < file` / stdin script | the same, on a temp file |
| `-m MODULE`, `-i`, no args (REPL), `-X`, `-W`, any other flag | `exec python3` with the same argv |
| `BREVI_SHIM=0` set | `exec python3` always |

- Brevi's `{"path":...}` line and refusal reasons go to stderr only when `BREVI_SHIM_VERBOSE=1`, so a harness that
  parses stderr sees exactly what CPython would print.
- With `BREVI_SHIM_VERIFY=1` each run is `brevi run --verify` (VERIFY_BY_DEFAULT.md), for harnesses that want the
  certificate.
- `sys.executable`, `sys.argv[0]` and `__file__` must read as CPython's would; a program that inspects them is sent to
  CPython. These are conformance tests before the shim ships.
- Installed as `brevi-python` by the wheel and the release archive. **Never** installed as `python3` on a user's
  machine. Only the base images (below) may put it first on PATH as `python3`, and they say so in their README.

## 3. Base images

One image, built from `templates/image/Dockerfile`, published by the release workflow and used by every template.

- **Registry:** `ghcr.io/ets-public/brevi-sandbox:VERSION` and `:latest`, linux/amd64 first (arm64 later if a
  platform asks). Built by a Linux job in `release.yml` on Mondays only, so the cost is one Docker build a week, about
  10 minutes of Linux time.
- **Contents:** Brevi, the minimal Rust toolchain it builds with, gcc and libc headers, python3, a warmed
  `BREVI_CACHE`, the SDK. Variants: `brevi-sandbox:VERSION` (python3 is CPython, `brevi-python` beside it) and
  `brevi-sandbox:VERSION-python3` (the shim is `python3`, CPython is `python3.real`).
- **Platforms:** E2B (template built from the image), Daytona (snapshot from the image), Modal
  (`Image.from_registry`), Cloudflare Sandbox (its own Dockerfile already matches the image body, checked by
  `templates/test.py`), plain Docker. Each template switches from "build the image yourself" to the published tag.
- **Size:** about 1.4 GB today, mostly the toolchain. Shipping the prebuilt runtime rlib (WS2) and stripping docs and
  unused targets from the toolchain should bring it under 700 MB; a dev backend without `rustc` (WS2 spike) would
  remove the toolchain from the image entirely.
- **Upstream:** once the image has run on each platform end to end, offer each platform a PR to their template
  gallery or docs. This is where the alpha outreach (drafted for Adam) starts.

## 4. GitHub Action and devcontainer feature

- `ets-public/brevi/setup@vX`: installs the release binary (checksum-verified) and the toolchain on a runner, with
  the cache keyed on the Brevi version. For CI agents.
- A devcontainer feature `ghcr.io/ets-public/features/brevi`, from the same install script.

## 5. Install to first run, measured

`tools/install_timer.sh` (to write): from a clean `debian:trixie-slim` container with no Rust, time
`docker pull` + `docker run ... brevi run hello.py` printing `45`, and separately the `pip install brevi` path
(which still needs a toolchain today). The roadmap target, 60 s or less, is met by the image path once it is
published; the pip path meets it only with the dev backend.

## Order of work

1. Python SDK over `brevi mcp`, with tests and a 10-line README example.
2. Publish the image to GHCR from `release.yml`; switch the templates to it; run each template end to end where we
   have an account (Docker and Cloudflare today; E2B, Daytona and Modal need Adam's accounts).
3. `brevi-python` shim, behind its conformance tests.
4. Node SDK, Action, devcontainer feature.

## Open questions

1. GHCR or Docker Hub? Proposed: GHCR, since it needs no new account and the workflow token can push to it.
2. Should the SDK default to `verify=True` when `sandbox=False`? Proposed: yes, matching the MCP default in
   VERIFY_BY_DEFAULT.md.
