# Upgrading Brevi

Brevi is 0.x and a new minor release ships most Mondays. Only the latest minor gets every fix, so keep current. See [SUPPORT.md](SUPPORT.md). The full list of changes is [CHANGELOG.md](../CHANGELOG.md).

## See your version

```
brevi --version
```

## Upgrade, by install channel

| Channel | Upgrade | Pin a version |
| --- | --- | --- |
| `install.sh` | run the install command again (see the README) | `BREVI_VERSION=0.27.0 sh install.sh` |
| uvx | `uvx --python 3.13 brevi@latest run --verify main.py` (plain `uvx` reuses its cached copy) | `uvx --python 3.13 brevi@0.28.2 run --verify main.py` |
| uv tool | `uv tool upgrade brevi` | `uv tool install --python 3.13 brevi==0.28.2` |
| pip | `pip install --upgrade brevi` | `pip install brevi==0.27.0` |
| npm | `npm install -g brevi@latest` | `npm install -g brevi@0.27.0` |
| cargo | `cargo install brevi --force` | `cargo install brevi --version 0.27.0 --force` |
| Docker image | pull `ghcr.io/ets-public/brevi-sandbox:latest` | use a tag such as `ghcr.io/ets-public/brevi-sandbox:0.27.0` (from v0.27.0) |
| GitHub Action | `ets-public/brevi/setup` at a newer tag | `uses: ets-public/brevi/setup@v0.27.0`, or the `version` input |
| Dev container feature | set `version` to `latest` (the default) | `"version": "0.27.0"` |
| Python and Node SDKs | upgrade the pip or npm package | pin the package version |
| Windows | unzip `brevi-x86_64-pc-windows-msvc.zip` (or `brevi-aarch64-pc-windows-msvc.zip` on ARM64) from the release over the old `brevi.exe` | download that release's zip |

Notes:

- `install.sh` puts `brevi` and `brevi-python` in `~/.cargo/bin` and checks the download against the release's `SHA256SUMS`. `BREVI_FROM_SOURCE=1` builds from source instead.
- The npm wrapper downloads the matching release binary on first run into `~/.cache/brevi/VERSION`, so a new package version fetches a new binary.
- The Action, the dev container feature and the sandbox image exist from v0.27.0.
- The pip, npm, cargo and Docker pin forms are those tools' usual syntax. The tag names are the release versions.
- Brevi's build cache is keyed on the compiler, so a new release does not reuse binaries built by an old one.
- After upgrading, run `brevi verify x.py --input tests/` on your own programs. It prints a certificate when the native build and CPython agree.

## Behaviour changes by version

Newest first. Only changes that can affect programs or setups that already work.

### v0.28.3

These changed in v0.28.3 (19 Nov 2026).

- **Unknown options are refused.** `brevi build`, `check`, `emit`, `test` and `fix` exit 2 on an option they do not have (before, `--sandbx` was dropped and the build ran unsandboxed), naming the nearest real one. Words after `--` go to the program.
- **Imports hidden on a line are refused under `--sandbox`.** An import after `;`, after a one-line `if`/`def`, or split with `\` must move to a line of its own (or the caller grants `--allow native`). Programs can no longer import Brevi's internal `_brevi_*` modules.
- **`--allow-write` over Brevi's caches is refused.** A write grant (and MCP `allow_write`) over a folder that holds or lies inside the build cache, Brevi's temp folder or `BREVI_PY_CACHE` is refused, so `--allow-write ~` or `--allow-write /tmp` fails where those hold them: grant a narrower folder. A cache another user owns or can write is not used; Brevi says so and uses a private one.
- **Crate builds use only a cargo home you own.** `CARGO_HOME` (else `~/.cargo`) is used only if root or you own it and nothing directly in it is writable by others; otherwise Brevi says so and uses a private one (crates are fetched again). cargo no longer reads a `.cargo/config.toml` from the folder you build in. The sandbox image no longer sets `CARGO_HOME`.
- **Held CPython cannot reach other processes (Linux).** Under `--sandbox` with `--allow-py` or `--allow native`, `ptrace`, `process_vm_readv/writev`, `pidfd_open`, `pidfd_getfd` and `userfaultfd` fail with `PermissionError`, and a kernel that refuses the filter refuses the program even with the network granted.
- **Crate lines are checked.** A version must be a Cargo version requirement and features Cargo feature names, and under `--sandbox` every crate needs a version: add `use NAME "VERSION"` for a crate the rust code names without one.
- **`brevi mcp --http` without a token answers only local Host names** (localhost, 127.0.0.1, [::1]). A tunnel that forwards to 127.0.0.1 with its own host name (cloudflared does by default) needs `--allow-host NAME`, or better a token. With a token, five wrong tokens in a row from one address get 429 for a growing wait (up to 60 s).
- **Sandboxed CPython is refused where the kernel cannot lock it.** Under `--sandbox` with `--allow-py` or `--allow native`, a program that runs CPython is refused on macOS and Windows at build time, and at start on a Linux kernel without Landlock (or without seccomp, or on an architecture other than x86-64 and arm64, when no network is granted). Before, CPython there kept full file access when no folders were named, and the network on macOS and Windows. Grant `--allow read,write` and `--allow net` if CPython should have them, or pass `--allow-unlocked-python` (`brevi mcp --allow-unlocked-python` for a server) to build it anyway: it then warns on every start.
- **`--limit cpu` works on Windows**, held by a Job Object and a watch thread over user and kernel time. `file`, `fds` and `threads` stay refused there.
- **`BREVI_DYN=0` keeps the wrong-answer guard.** A file the guard sends to the dynamic build is refused instead of built typed.
- **Pickle and copy refusals stop the program** instead of raising an exception a `try` could catch. Pickles of the program's own classes, int and tuple subclasses and namedtuples now have CPython's exact bytes, and `copy` honours a class's `__reduce__`, `__getstate__` and `__setstate__` (they were skipped before).
- **numpy with `NPY_*` or `NUMPY_*` settings** in the environment runs with CPython's numpy, not Brevi's.
- **Typed build:** a loop over a JSON number, bool or None raises Python's `TypeError` (it looped zero times), and `split()`, `strip()` and `isspace()` treat U+001C to U+001F as whitespace.

### v0.28.2

These changed in v0.28.2 (16 Nov 2026).

- **Private temp folders (Linux and other unix).** Build folders, cargo projects, MCP work folders and the background build slots live in `$TMPDIR/brevi-UID` (else `~/.cache/brevi/tmp`), used only when it is a real folder owned by you with no access for anyone else. If neither can be used, brevi stops and says why instead of falling back to a shared name. With no home folder, the cache moves into the private folder too.
- **`brevi mcp --http` closes slow or oversized requests.** A body past 4 MiB (chunked ones included) gets 413, a connection silent for 30 s or a request not whole within 120 s is closed, and past 64 connections at once the server answers 503.
- **Windows:** CRLF source reads as CPython reads it (no `\r` kept in triple-quoted strings); source paths have no `\\?\` prefix; the cp1252 codec works and bytes `re.LOCALE` follows the C library's locale. With `PYTHONUTF8=1`, bridged CPython runs in UTF-8 mode, so `locale.getpreferredencoding()` is `utf-8`, as in python3.
- **No `rustc` on the PATH no longer means no build** (Linux x86-64, macOS). brevi fetches the pinned Rust release it is tested with, and zig to link when there is no working C linker, each checked by SHA-256 and kept in Brevi's own folder; `install.sh` no longer installs rustup there. Set `BREVI_TOOLCHAIN=system` to keep the old behaviour (use only the Rust on PATH). A `rustc` already on the PATH is used as before.
- **Bytes patterns with `re.LOCALE`** take case and word characters from the environment's LC_CTYPE locale on every platform, as CPython does. Under `--sandbox` such a match stops the program.

### v0.28.1

These changed in v0.28.1 (16 Nov 2026).

- **Unicode case follows CPython 3.13.** `str.upper()`, `lower()`, `title()`, `capitalize()`, `swapcase()` and the `is*` case and space tests use CPython 3.13's Unicode 15.1 tables in both builds, with its title-case, final-sigma and word rules. Characters added to Unicode after 15.1 and a few title-case letters gave different answers before. `\x1c` to `\x1f` count as whitespace in `isspace()`, `split()` and `strip()`.
- **`re.IGNORECASE` builds on any Rust.** It folds case with CPython 3.13's tables, so Unicode IGNORECASE patterns no longer refuse on Rust 1.100 and newer. Building Brevi from source is tested on Rust 1.100.0.
- **`namedtuple` names** are checked with CPython 3.13's `str.isidentifier()` (`'a²'` is refused and `'a·b'` accepted, as in CPython).
- **Regex patterns built at run time** in a typed build of a .py file are checked like literal ones: syntax the typed matcher lacks (back-references, named groups, look-arounds) stops the typed run, and `brevi run` re-runs it on the dynamic build.
- **`--standalone` binaries use only the Python they carry.** One whose bundle is missing now refuses to run instead of using an installed Python, and dynamic `--standalone` builds carry CPython too. With `--standalone`, duckdb, onnxruntime, the network modules and (Linux, macOS) sqlite3 run in the bundled CPython.
- **Windows:** `--standalone` works; HTTPS trusts the Windows certificate store when no CA file is given; ARM64 builds ship (`brevi-aarch64-pc-windows-msvc.zip`, `win_arm64` wheel, npm `win32-arm64`); a release is gated on Windows conformance.
- **Conformance floors count native matches only** (a match through the CPython bridge no longer counts), and 4 `test_set.TestBasicOpsTriple` results are unstable on every platform, so core is scored over 1072 tests.

### v0.28.0

These changed in v0.28.0 (15 Nov 2026).

- **Sandboxed CPython has no network without a grant (Linux).** Under `--sandbox`, a program that loads CPython (`--allow-py`, `--allow native`) and has no network grant gets `PermissionError` from `socket()`, in CPython, its C extensions and every process they start. A sandboxed program that starts CPython with hosts named by `--allow-net` is refused on every OS, because CPython cannot be held to those hosts. Grant `--allow net`, or leave CPython out.
- **DuckDB and ONNX Runtime libraries are pinned to the build.** A program loads the library only if its bytes still match the ones it was built against. After upgrading duckdb or onnxruntime, `brevi run` rebuilds by itself, but a binary from `brevi build` must be built again.
- **Pickling and copying iterators.** itertools iterators, `map`, `filter`, `zip`, `enumerate`, `reversed` and `functools.partial` pickle, copy and deep-copy as in CPython 3.13, with CPython's DeprecationWarnings for itertools. Before, some were saved as a plain list iterator, and `copy.copy` of a list, tuple, str, range, dict or set iterator returned the same iterator. Copying a generator raises `TypeError`, as in CPython.
- **Abstract methods are enforced.** In dynamic builds, instantiating an `abc.ABC` (or `metaclass=ABCMeta`) class that still has an abstract method raises CPython's `TypeError`. Before, the object was made.
- **Annotations are evaluated.** Dynamic builds evaluate module, class and def annotations as CPython 3.13 does, so `x: undefined_name` raises `NameError` and a bad subscript such as `int[str]` raises `TypeError`. The typed build sends a file whose annotations could raise to the dynamic build.
- **Builtin types answer only CPython's attributes.** `hasattr(int, "zzz")` is False and `int.zzz` raises `AttributeError` (a method stub came back before).
- **Refused at build time now:** `typing.ByteString`, and a class built on `zip`, `map`, `filter`, `enumerate`, `reversed` or an itertools type (these stopped at run time before). `collections.abc.ByteString` issues CPython 3.13's DeprecationWarning.
- **PEP 479 for generators.** In dynamic builds a `StopIteration` that escapes a generator or a generator expression raises `RuntimeError("generator raised StopIteration")`, as in CPython. Before, the generator ended quietly.
- **`is` and `id()`.** The typed build no longer answers `is` by comparing values (`[1] is [1]` was True): a file that uses `is` with anything but None, True, False or Ellipsis goes to the dynamic build, where `id()` and `is` now agree for every value (`id()` of a float, set, function or iterator was often 1).
- **OrderedDict and deque.** Pickling an OrderedDict gives back an OrderedDict (a plain dict before); calling dict's own methods on one (`dict.pop(od, k)`) is refused. Changing a deque or OrderedDict while iterating it raises, and `insert` into a full deque raises `IndexError`, as in CPython. `list.insert` past either end clamps in typed builds too.
- **`match` statements build natively** in the dynamic build, so `brevi build` and `--native` no longer refuse them; a pattern CPython's compiler rejects refuses the build.
- **New refusals at build time** (dynamic builds): `import dis`; `locals()` in the body of a class that may be an Enum; a class built on `datetime.date`, `datetime`, `time`, `timedelta` or `timezone`; reading `__bases__`, `__mro__`, `__dict__`, `dir()` or `vars()` of a class on `Generic` or `Protocol`; `issubclass()` with a Protocol; a module's `__spec__`, `__loader__` and `__dict__`. `brevi run` runs these with python3; `--strict` refuses them.
- **Module reprs and `__file__`.** A program module's repr is CPython's (`<module 'helper' from '/abs/helper.py'>`) and its `__file__` and `__doc__` now read; a library module's repr is the one the build machine's CPython prints. On Windows `__file__` no longer starts with `\\?\`.
- **Threads take turns.** In dynamic builds each `threading.Thread` runs on its own stack, and `Event`, `Lock`, `RLock`, `join()` and `time.sleep()` wait as in CPython. Before, `start()` ran the whole thread at once. A thread that never waits prints what it printed before.
- **Windows paths are Windows paths.** In dynamic builds on Windows, `os.path` is `ntpath` (drive letters, both slashes, UNC shares), `os.sep` and `os.linesep` are Windows', and `sys.platform` is `win32` (`darwin` on macOS; it said `linux` everywhere before). A program on Windows that relied on POSIX path rules now gets CPython's answer. `pathlib` on Windows runs in CPython (refused with `--strict`).
- **More CPython warnings.** Dynamic builds issue CPython 3.13's DeprecationWarning and FutureWarning where CPython does (`int()` of an `__int__` returning a bool, `functools.partial` read from an instance), naming the program's own line. `warnings` filters in the program hide them, as in CPython.
- **Releases are held to the conformance gates.** Nothing is published unless core conformance is at least 90%, stdlib at least 80%, and neither has a known-wrong result.
- **New `--strict`** (or `BREVI_STRICT=1`) refuses every fallback. It changes nothing unless you pass it ([Strict builds](https://brevi-lang.org/docs/usage#strict-builds)).

### v0.27.0

- **MCP sandbox is locked by default.** A plain `brevi mcp` refuses a call with `sandbox: false`. A call can grant only what you passed to `brevi mcp` (`--allow`, `--allow-py`, `--allow-net`, `--allow-read DIR`, `--allow-write DIR`); the default is nothing. `--trust-calls` restores the old behaviour, and the `verify` tool needs it. The Python and Node SDKs start it that way, so they work as before.
- **Sandboxed programs start with a short environment.** Only `PATH`, locale, `TZ`, terminal size, temp folders and Brevi's own `BREVI_*` settings. With `--allow-py` or `--allow native`, also the home folder and proxy and certificate settings. If a sandboxed program read a key from the environment, pass it another way.
- **Sandboxed runs are checked against Brevi's dynamic build.** The MCP `run` tool and `brevi run --verify --sandbox` compare the typed answer with the dynamic build. When they differ you get the dynamic answer and a `{"verify":"differs",...}` line.
- **Limits.** `--limit written` now holds to the byte and needs `--sandbox` without `--allow-py` or `--allow native`. `--limit threads` is held by a cgroup. Under `written`, SQLite, DuckDB and ONNX Runtime open database files read-only and write no files.
- **Arguments.** `brevi run file.py a b` passes `a b` to the program. A dashed word after the file that is not a Brevi option is refused with exit 2; put it after `--` to pass it on.
- **Big ints.** On the typed build an int that passes 64 bits no longer wraps. `brevi run` re-runs such a program from the start on the dynamic build.
- **Windows is beta.** Programs that import libraries now build there. Text output uses CRLF as CPython does. See [KNOWN_ISSUES.md](KNOWN_ISSUES.md) for what is refused.
- **MCP argument checks.** An `allow_read` or `allow_write` item with a comma is refused. `code`, `patch` and `text` must be strings.

### v0.26.0

- **New limits.** `--limit` also takes `written`, `threads`, `fds`, `files` and `net`. A limit the platform cannot enforce is refused, not dropped.
- **Folder-level grants.** `--allow-read DIR` and `--allow-write DIR` hold a program to folders. `--allow read` and `--allow write` still mean every file the user can reach.
- **CPython is held to file grants on Linux** under `--sandbox` (Landlock, Linux 5.13 or later). Where Landlock is missing, named folders are refused.
- **numpy needs no grant under `--sandbox`.** Hugging Face `Tokenizer.from_file` now needs a read grant, and `from_pretrained` a network grant.
- **`brevi mcp --http`** needs a bearer token (at least 16 characters) on any address other than 127.0.0.1.
- **Decorators and `json` options** that typed builds used to ignore now work or are refused. Programs that relied on the old wrong output change.

### v0.25.0

- **Uncaught exceptions in `.py` files exit 1**, as in CPython. Native typed builds used to exit 101. `.bv` programs still exit 101.
- **A `.py` file with Brevi-only syntax is refused** (`&&`, `=>`, `fn`, `true`, `.push`), so `brevi run` falls back to CPython.
- **`install.sh` stops on a checksum mismatch** or when it cannot verify the download.
- Many silent wrong answers on the typed path were fixed. A program that printed the old answer now prints CPython's.

### v0.24.6 (breaking)

- **Under `--sandbox`, a program's own `allow` lines grant nothing.** Only the caller grants: `--allow`, `--allow-py`, or the MCP `allow` and `allow_py`. A program that asks for more than the caller granted is refused. Move the grants from the file to the command.
