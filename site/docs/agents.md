# For AI agents
<!-- description: Connect a coding agent to Brevi: the MCP server, the Agent Skill, the Claude Code plugin, editor rules, llms.txt, and how to prompt for the cheapest working program. -->

Brevi is built for the agent in the loop. The agent writes the plain Python it already knows, Brevi builds and runs it, and every problem comes back as one short line. There is nothing new for the model to learn, and no guide to load.

## The loop

1. Write normal Python to `main.py`. Standard library, or installed packages; split big programs into `.py` files that import each other.
2. Run it: `brevi run main.py < input.txt`, or `brevi check main.py` to build without running.
3. Each error is `{"line":N,"msg":...,"fix":...}`. Change only the lines it names, then run again. Do not rewrite the whole file.
4. If Brevi still cannot build a construct after one small change, keep the Python as it is and run it with `python3`. `brevi run` already does this on its own, with a one-line `{"path":"cpython",...}` report.

That is the whole contract, and it is what the skill, the MCP server and llms.txt all tell an agent.

## Write the plain solution

The one piece of prompting advice that matters: **ask for the plain, direct solution, and say it will be compiled.** Models that expect CPython hand-optimise: bit-packed integers, clever caching, vectorised tricks. That costs output tokens, adds bugs, and is often slower under Brevi than the obvious loop.

Brevi's skill, MCP server, editor rules and llms.txt all carry this advice. The benchmark prompt uses this exact line:

> Your program is compiled to native code, which runs plain loops over ints and lists 10 to 50 times faster than CPython. Write the plain, direct solution; do not hand-optimise it for speed.

On four heavy tasks (Game of Life, knapsack, edit distance, Collatz chains), adding that line cut Sonnet's output tokens by half and Haiku's by over a quarter, and the total cost came to 0.58x (Sonnet) and 0.53x (Haiku) of the same models writing Python for CPython. Every plain program ran natively within the 10-second limit, where CPython needed 29 to 59 seconds. On ordinary tasks the line makes no clear difference either way. Details in [Benchmarks](/docs/benchmarks#heavy-tasks-and-the-plain-solution-line).

## MCP server

`brevi mcp` is a Model Context Protocol server over stdio. It speaks every protocol version up to the newest: 2026-07-28 (from the next release; `server/discover`, no handshake) and the handshake versions 2024-11-05 to 2025-11-25. Add it to any MCP client as the command `brevi` with the argument `mcp`.

**Claude Code:**

```
claude mcp add brevi -- brevi mcp
```

Check it with `claude mcp list` (it should say `brevi: ... Connected`).

**Claude Desktop, Cursor, and other clients** with a JSON config (for Claude Desktop, `claude_desktop_config.json`, opened from Settings, Developer, Edit Config):

```
{
  "mcpServers": {
    "brevi": { "command": "/Users/you/.cargo/bin/brevi", "args": ["mcp"] }
  }
}
```

Use the full path that `which brevi` prints (on Windows, `where brevi`, written with doubled backslashes: `"C:\\Users\\you\\.cargo\\bin\\brevi.exe"`). Desktop apps do not start servers with your shell's PATH, so a bare `brevi` is often not found. Brevi itself then finds `rustc` in `~/.cargo/bin` even when the client's PATH leaves it out. Restart the client after editing the file.

**Docker**, with nothing installed locally: `docker run --rm -i brevi mcp` (see [Install](/docs/install#docker)).

The server **keeps the program**. The agent sends its code once, then fixes it with `patch` or `edit`, which change only what they are sent and check again in the same call. A retry costs only the lines that change, not the whole file.

| Tool | What it does |
| --- | --- |
| `check` | Compile without running. Returns `OK` or one JSON error per line. Takes `code` (once), `file` (for multi-file programs), `fast`, `fix` and `sandbox`. |
| `run` | Compile and run the kept program with `stdin` and `args`. With `cases` (a list of `{stdin, expect}`), builds once and prints `ok` or the first differing line for each case. `timeout` gives a slow program more time. |
| `test` | Run the `test` blocks of a Brevi program. |
| `verify` | Proof the native build answers as CPython does: builds the kept program natively, runs it and `python3` on each of `inputs` (stdin texts) with `args`, and compares stdout, exit code and stderr. One JSON line per input, then a certificate (`identical`, `differs`, `unstable`, `timeout` or `not built`, with the file's SHA-256 and the speedup). CPython runs twice per input (`once: true` for one run). It runs CPython on the code, which the sandbox cannot hold, so it needs `sandbox: false`. From the release after v0.25.0. See [Verify against CPython](/docs/usage#verify-against-cpython). |
| `patch` | Replace each `def` or `class` (in Brevi code: `fn`, `type`, `enum` or `test`) by name, or add it, then check. |
| `edit` | Replace lines `from`..`to` of the kept program with `text`, then check. |
| `show` | The kept program, or some of its lines, with line numbers. |
| `guide` | The Brevi language guide, or only the lines about one word. Not needed for Python. |

The tools sandbox by default, and only the caller grants capabilities: pass `allow: ["read"]` (or `"write"`, `"native"`) for a program that touches files, and `allow_py: ["requests"]` for one that imports a Python library CPython runs (numpy, requests, subprocess, ...). An `# allow` line in the code grants nothing under the sandbox (from v0.24.6), so code the agent writes cannot free itself. Path text (`os.path.join`, `pathlib.PurePath`, ...) and `random` (seeded draws match CPython) work without it from v0.24.2. Pass `sandbox: false` to turn the sandbox off.

**Every `check`, `run` and `patch`/`edit` result says which path built the program**, in the result's `structuredContent`: `{"path":"typed"}` on the fast path, or `pybridge`, `dynamic` or `cpython` with `why`, `line`, `fn` (and `file`) naming the code that kept it off the typed path. When the path is not `typed`, the same JSON line is also the last line of the text, so a client that ignores structured results still sees it. To get the typed path back, rewrite only the function it names.

### Limits

Each call gets 20 seconds for build and run together (each `cases` entry 10 seconds); a tool call can pass `timeout` (up to 600), and `brevi mcp --timeout SECONDS` changes the default. On Linux a program may use at most half the machine's memory (`brevi mcp --mem-mb MB`, 0 for no limit). When a limit is reached, or the client cancels the call, the program and everything it started are stopped, and the reply says which limit it was. Long output is cut in the middle: the reply keeps the first 16 KB and the last 4 KB. Nothing is left behind: the server writes the program to one temporary folder and removes it, with its builds, when the client disconnects.

The first call after installing or upgrading Brevi can take up to half a minute: the server builds Brevi's runtime libraries once, in the background, as soon as it starts.

### If it does not connect

- `rustc not found`: install Rust with [rustup](https://rustup.rs), then restart the client.
- The client says the server failed to start: use the full path to `brevi` in its config (above), and check that `brevi --version` works in a terminal.
- To see what the server sends, run it by hand: `brevi mcp`, then paste `{"jsonrpc":"2.0","id":1,"method":"tools/list"}` and press Enter. It answers with one line of JSON.

## Agent Skill and Claude Code plugin

`skills/brevi/SKILL.md` is an [Agent Skills](https://agentskills.io) package: setup (check `brevi --version`, install if missing), then the loop above. Copy the `skills/brevi` folder into your agent's skills folder (for Claude Code, `~/.claude/skills/` or a project's `.claude/skills/`). The text is on this site as [skill.md](/skill.md).

The repository is also a Claude Code plugin marketplace. The plugin adds the skill and the `brevi mcp` server in one step (it needs `brevi` on the PATH):

<!--public-->
```
/plugin marketplace add ets-public/brevi
/plugin install brevi@brevi
```
<!--/public-->
<!--private-->
```
/plugin marketplace add ets-public/brevi      # works once the repository is public
/plugin install brevi@brevi
```
<!--/private-->

## Editor rules

`skills/rules/` has ready-made rules for Cursor (`cursor/brevi.mdc`), GitHub Copilot (`copilot/brevi.instructions.md`), Windsurf and Cline, generated from the same text as the skill. Copy the one for your editor into its rules folder.

## llms.txt and Markdown docs

- [/llms.txt](/llms.txt): a short index of what an agent should read, starting with the skill.
- [/llms-full.txt](/llms-full.txt): the skill, guide, reference and library notes in one file.
- Every page of these docs is also plain Markdown: add `.md` to its address (for example [/docs/usage.md](/docs/usage.md)), or use the "View as Markdown" link at the top of each page.

## Agent frameworks

The repository's `integrations/` folder has short, tested examples that connect `brevi mcp` to LangChain, the OpenAI Agents SDK, CrewAI and the Vercel AI SDK.

## Errors an agent can act on

Errors are designed around what the next model call needs: the line, the problem in plain words, and a fix when Brevi knows one. No file paths, columns, error codes or generated Rust. A typical Brevi error is 20 to 50 tokens where a traceback or `rustc` message is 80 to 160. See [Errors](/docs/usage#errors) for the formats.

## Constrained decoding

For local models, `grammar/` has a Lark grammar of the Brevi language and a llama.cpp GBNF copy, so a model can only write lines Brevi accepts. This applies to `.bv` programs, not Python.

## Coming: brevi.run

A hosted version of the same tools over HTTPS, for agents that cannot install a toolchain: send a program, get the output, test results or error lines back. See [brevi.run](https://brevi.run).
