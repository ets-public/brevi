# Support matrix

## Resource limits by platform

Where each `--limit` is enforced. A limit marked `-` is refused on that platform, never ignored. `written`, `net` and `files` are counted by Brevi's own code, so they also need `--sandbox` without `--allow native` or `--allow-py`. `threads` also needs a pids cgroup Brevi can create. Source of truth: `LIMIT_PLATFORMS` in `src/proc.rs`; `tools/limits_check.py` checks this table.

| Limit | Linux | macOS | Windows |
| --- | --- | --- | --- |
| `cpu` | yes | yes | - |
| `mem` | yes | - | yes |
| `time` | yes | yes | yes |
| `output` | yes | yes | yes |
| `file` | yes | yes | - |
| `written` | yes | yes | yes |
| `net` | yes | yes | yes |
| `threads` | yes | - | - |
| `fds` | yes | yes | - |
| `files` | yes | yes | yes |

## Paths

Every build of a Python file takes one of four paths and says which in one JSON line on stderr (`{"path":...}`; see [ARCHITECTURE.md](ARCHITECTURE.md#which-path-ran)). The MCP tools return it as `structuredContent`.

| Path | Native | CPython | `--native` | `--strict` |
| --- | --- | --- | --- | --- |
| `typed` | yes | no | passes | passes |
| `pybridge` | yes, with the imported libraries in CPython (libpython) | the libraries named in `py` | passes | passes |
| `dynamic` (fallback) | yes, over dynamic values | only libraries named in `py` | passes | refused |
| `cpython` (fallback, `brevi run` only) | no | the whole program | refused | refused |

Under `--sandbox` (and in the MCP tools by default) nothing runs in CPython unless the caller grants it (`--allow-py`, `--allow native`), and a sandboxed program never takes the `cpython` path.

### What takes which path

| Construct | Path | Notes |
| --- | --- | --- |
| Plain Python with one type per value (numbers, str, lists, dicts, sets, classes, generators, `async`) | `typed` | The fast path; the speed figures come from it. |
| A value that changes type, mixed-type lists, lists shared between names, a set printed or walked in hash order | `dynamic` | The guard sends it there, as the typed answer would differ from CPython. |
| An int that outgrows 64 bits at run time | `typed`, then `dynamic` | `brevi run` runs it again on the dynamic build; `--strict` stops instead. |
| numpy, requests, httpx, pydantic, the LLM SDKs, `mcp`, jinja2, polars, sqlite3, duckdb, tokenizers, safetensors, onnxruntime | `typed` or `dynamic` | Brevi's own native copies. |
| Other installed libraries (pandas, yaml, ...) | `pybridge` (or `dynamic` with `py`) | Library calls run in CPython inside the native program; Linux, macOS and Windows (Windows from v0.27.0). |
| Classes built on a library's classes | `dynamic` with `py` | CPython runs the class as a whole. |
| `match`, `eval`/`exec` of text known only at run time | `cpython` | `brevi build`, `check`, `--native` and `--strict` refuse them. |

### Platforms

| | Linux x86-64 | macOS | Windows x86-64 |
| --- | --- | --- | --- |
| typed and dynamic builds | yes | yes | yes, beta (from v0.27.0) |
| pybridge (CPython loaded into the program) | yes | yes | yes, needs the Python DLLs (from v0.27.0) |
| `--standalone` | yes | no | no |
| Landlock holding CPython to file grants | Linux 5.13+ | no | no |
| Resource limits | see the table above | see the table above | see the table above |
