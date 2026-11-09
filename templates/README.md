# Brevi on agent platforms: E2B, Daytona, Modal, Cloudflare

Templates for running the Python an agent writes inside a hosted sandbox, compiled and held by Brevi. The
platform gives each run a throwaway machine. Brevi adds two things a VM cannot see:

- **Capabilities the caller grants.** Under `--sandbox` the program may not read or write files, open
  connections, start processes or hand modules to CPython unless your code grants it (`--allow read,write,net,native`,
  `--allow-net HOST`, `--allow-py MODULE`). A refused program does not run at all, and the error says what it
  needed. See [THREAT_MODEL.md](../docs/THREAT_MODEL.md).
- **Limits the caller sets.** `--limit cpu=10,mem=512,time=20,output=256,file=16` stops a runaway program and says
  which limit stopped it (`{"e":"limit","on":"mem",...}`, exit code 124), well before the platform's own timeout.

The model never chooses either. `brevi_tool.py` gives it one tool, `run_python(code, stdin)`, with no field
for permissions; grants and limits come from your code.

| Platform | Template | Isolation | Setup |
| --- | --- | --- | --- |
| [E2B](https://e2b.dev) | [e2b/](e2b/) | Firecracker microVM | `pip install e2b`, `E2B_API_KEY`, then `python templates/e2b/build_template.py` once |
| [Daytona](https://daytona.io) | [daytona/](daytona/) | sandbox from a snapshot | `pip install daytona`, `DAYTONA_API_KEY`, then `python templates/daytona/build_snapshot.py` once |
| [Modal](https://modal.com) | [modal/](modal/) | gVisor sandbox | `pip install modal`, `modal setup`; the image builds on first use |
| [Cloudflare](https://developers.cloudflare.com/sandbox/) | [cloudflare/](cloudflare/) | Container behind a Worker | `npm install`, `npx wrangler secret put BREVI_TOKEN`, `npx wrangler deploy` |
| Docker, your machine | [docker/](docker/) | container | `docker build -t brevi-sandbox templates/image` |

All of them run the same image, [image/Dockerfile](image/Dockerfile): Debian, the Rust toolchain Brevi builds
with, `brevi` from the latest release (checked against its SHA256SUMS), and `python3` for `brevi verify`. The
Cloudflare image is the same file plus the shim the Sandbox SDK needs. Give each sandbox about 2 GB of memory:
rustc needs it to link a program.

## Run an agent

```sh
pip install anthropic
export ANTHROPIC_API_KEY=...
python templates/agent.py --platform docker "How many primes are there below 10 million?"
python templates/agent.py --platform e2b --allow-net api.github.com --limit cpu=5,mem=256,time=10 \
  "How many open issues does ets-public/brevi have?"
```

[agent.py](agent.py) is a plain Claude tool loop, about sixty lines: the model writes Python, calls `run_python`,
reads the exit code, stdout and stderr, and fixes what the error names.

## Use it in your own agent

```python
from brevi_tool import TOOL, Grants      # with templates/ on sys.path
from agent import platform
Sandbox = platform("e2b")                 # loads templates/e2b/sandbox.py; or "daytona", "modal", "cloudflare", "docker"

with Sandbox(Grants(allow_net=["api.github.com"], limits={"cpu": 5, "mem": 256, "time": 10})) as box:
    r = box.run(code, stdin)              # Result(exit_code, stdout, stderr)
    r.limit                               # "cpu", "mem", "time", "output", "file" or None
    r.for_model()                         # the text to hand back as the tool result
```

Pass `TOOL` to the model as a tool definition (it is in the Claude API's format; for OpenAI use its
`name`, `description` and `input_schema` as `parameters`). Each sandbox runs
`brevi run --sandbox [--allow ...] --limit ... main.py < stdin.txt` in `/workspace`.

The platform's network setting follows the grant: with nothing granted, E2B's VM gets no internet, Daytona's
sandbox blocks all traffic, Modal's sandbox has `block_network`, and the Cloudflare container starts without the
internet. With `--allow-net HOST`, Brevi lets the program connect only to the granted hosts, and on Modal the
sandbox's own domain allowlist holds the same hosts.

## Brevi's MCP server in a sandbox

From v0.26.0, the image can also serve `brevi mcp` over HTTP, so an MCP client outside the sandbox
(Claude, the Agents SDK, any Streamable HTTP client) uses Brevi's tools without starting a process:

```sh
brevi mcp --http 0.0.0.0:8000 --token "$BREVI_MCP_TOKEN"     # inside the sandbox
```

Then point the client at the platform's address for port 8000, path `/mcp`, with the header
`Authorization: Bearer <token>`: E2B's `sandbox.get_host(8000)`, Daytona's `sandbox.get_preview_link(8000)`,
Modal's `encrypted_ports=[8000]` tunnel, or a Cloudflare container port behind the Worker. The token is
required on any address other than 127.0.0.1. Run it as a background process of the sandbox (E2B
`commands.run(..., background=True)`, Daytona sessions, Modal `Sandbox.exec`). This has been tested locally
with the official MCP client, not yet on the platforms themselves.

## Check the templates

```sh
python3 templates/test.py            # static checks: no Docker or account needed
python3 templates/test.py --docker   # also runs programs in the image: refusals and each limit
```

Tested on 7 Nov 2026: the image built and every `--docker` case passed (reads stdin, a dynamic program,
refused file read, network and subprocess, the memory, time and output limits, and a granted read, as a
non-root user); the Cloudflare Worker answered under `wrangler dev` with its real container (a program, a
refused read, the memory limit, a wrong token); and `agent.py --platform docker` with Claude Haiku 5.5 solved a
task and reported the refused read. The E2B, Daytona and Modal templates were checked against their SDKs
(e2b 2.63, daytona 0.230, modal 1.6; E2B parses the Dockerfile) but have not yet been run on those platforms,
which needs an account on each.
