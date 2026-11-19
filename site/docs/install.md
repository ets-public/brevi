# Install Brevi
<!-- description: Install Brevi on the machine that builds your binary, on Linux, macOS or Windows: uvx, install script, pip, npm, Docker or source. -->

Install Brevi on the machine that builds. The machine that runs your binary needs nothing for a native build (a build that calls Python libraries needs Python 3.13 and the packages it was built against there, or `--standalone`).

Brevi turns programs into Rust and calls `rustc` to build them. You do not need to install Rust: when no `rustc` is on the PATH, brevi fetches the Rust release it is tested with on its first build, and, when the machine has no C linker either, zig to link with. Both are checked against SHA-256 sums compiled into brevi and kept in Brevi's own folder (see [The pinned toolchain](#the-pinned-toolchain)). This works on Linux x86-64 and macOS (Apple silicon and Intel) from v0.28.2, and on Linux ARM64 and Windows (x86-64 and ARM64) from v0.28.3.

## One command (with uv)

With [uv](https://docs.astral.sh/uv/) installed, this downloads brevi, fetches its pinned toolchain on the first build, and builds `server.py` into one binary:

```
uvx brevi build server.py --release -o my-server
```

The first build fetches about 300 MB once (Rust and zig) and builds Brevi's runtime, which takes one to two minutes; later builds take seconds. See [Ship an MCP server or CLI as one binary](/docs/bundle-mcp) for what to do with the binary.

To also check a run against CPython 3.13 (the version Brevi matches):

```
uvx --python 3.13 brevi run --verify main.py
```

`uvx brevi mcp` starts Brevi's own MCP server the same way.

## Requirements

| What | Why | Notes |
| --- | --- | --- |
| `rustc` (Rust stable) | Brevi compiles every program through it | Fetched by brevi when none is on the PATH (the pinned toolchain), where that is verified; elsewhere installed by the script with rustup. Any recent stable works; CI tests stable daily and beta and nightly weekly. |
| a C linker (`cc`) | rustc links programs with it | Not needed: with no working `cc` (Linux without the C library's development files, macOS without the Command Line Tools), brevi links with zig, fetched once. |
| CPython 3.13 (for `--verify`) | `brevi run --verify` and `brevi verify` compare each run with CPython 3.13, and use no other version | Brevi takes `BREVI_PYTHON` when set, else the first 3.13 among `python3`, `python3.13` and `python` on PATH, then (Windows) the `py` launcher, then `uv python find 3.13`, so `uv python install 3.13` is enough. |
| `python3` (optional) | The CPython fallback of `brevi run`, and programs that import libraries such as numpy | Not needed for programs that use only the standard library. Library programs need Python 3.13 and the packages they were built against (the build venv's site-packages) where they run, unless built with `--standalone`. |
| Linux, macOS or Windows | Prebuilt binaries for x86-64 and ARM64 Linux, Apple silicon and Intel Macs, and x86-64 and ARM64 Windows (ARM64 from v0.28.1) | Library calls through CPython work on Windows from v0.27.0 (the `python` on PATH needs its DLL); `--standalone` builds work on Linux and Windows from v0.28.1. |

## Linux and macOS: the install script

<!--public-->
```
curl -fsSL https://raw.githubusercontent.com/ets-public/brevi/main/install.sh | sh
```
<!--/public-->

The script:

1. downloads the prebuilt `brevi` for this machine from the latest GitHub release;
3. verifies the download against the release's `SHA256SUMS` (with `sha256sum` or `shasum -a 256`) and installs it into `~/.cargo/bin`;
4. builds it with `cargo install` instead when no release fits (set `BREVI_FROM_SOURCE=1` to always build from source; this needs Rust, which the script installs with rustup);
5. with no `rustc` on the PATH, runs `brevi toolchain` to fetch the pinned toolchain now, or installs Rust with rustup (minimal profile) where brevi cannot (a release before the pinned toolchain, or a platform it is not verified on yet);
6. prints `brevi --version`.

If the checksum does not match, or the download cannot be verified because `SHA256SUMS` or a checksum tool is missing, the script stops and installs nothing.

Check it worked:

```
$ brevi --version
brevi __FULLVERSION__
$ printf 'print(sum(range(10)))\n' > hello.py && brevi run hello.py
45
```

## Windows

1. From v0.28.3, skip this step: brevi fetches its own Rust and zig on the first build ([the pinned toolchain](#the-pinned-toolchain)). For v0.28.2 and earlier, install Rust with [rustup](https://rustup.rs); it also asks for the Microsoft C++ build tools, which `rustc` needs to link programs.
2. Download `brevi-x86_64-pc-windows-msvc.zip` (or `brevi-aarch64-pc-windows-msvc.zip` on ARM64 Windows, from v0.28.1) from the latest [GitHub release](__GITHUB__/releases/latest), unzip it, and put `brevi.exe` on your PATH (for example in `%USERPROFILE%\.cargo\bin`, next to `rustc`).
3. In PowerShell or cmd: `brevi --version`.

Standard-library programs build and run natively on Windows. From v0.27.0, Python files that import packages build too: their library calls run in CPython through the bridge, as on Linux and macOS (the `python` on PATH needs its DLL). `--standalone` works on Windows from v0.28.1.

## The pinned toolchain

When a build needs `rustc` and none is on the PATH, brevi downloads the Rust release CI tests with (the `rustc`, `rust-std` and `cargo` components, from static.rust-lang.org) and unpacks it into its own folder. When there is also no working C linker, it downloads zig from ziglang.org, and rustc links through it. Every download is checked against a SHA-256 compiled into brevi (`src/toolchain_pins.rs`); one that does not match is deleted, never unpacked. Nothing is installed system-wide and nothing on the PATH changes. Fetching needs `curl` and `tar` (and, on Linux, `xz` or `python3` to unpack zig).

| Platform | Pinned toolchain |
| --- | --- |
| Linux x86-64 and ARM64 | Used automatically (x86-64 from v0.28.2, ARM64 from v0.28.3). With a working `cc` it links with that, otherwise with zig. Not on a machine with an Arm Cortex-A53 core (such as a Raspberry Pi 3), whose erratum 843419 needs a linker fix zig cannot apply: there brevi asks for a C compiler or Rust. |
| macOS, Apple silicon and Intel | Used automatically (from v0.28.2). With Apple's Command Line Tools installed it links with them, otherwise with zig. |
| Windows x86-64 and ARM64 | Used automatically (from v0.28.3): the GNU-ABI (`gnullvm`) Rust, linked with zig, so neither Visual Studio nor its C++ build tools are needed. Programs use the Universal C Runtime that comes with Windows 10 and 11. |

| Setting | Effect |
| --- | --- |
| `brevi toolchain` | Fetches what is missing now (for an install script or a Docker layer) and prints one JSON line: the `rustc` and linker builds use. |
| `BREVI_TOOLCHAIN=auto` | The default: the `rustc` on the PATH, else the pinned one. |
| `BREVI_TOOLCHAIN=system` | Never fetch; fail when no `rustc` is on the PATH. |
| `BREVI_TOOLCHAIN=pinned` | Always use the pinned toolchain, even with Rust installed. |
| `BREVI_LINKER=cc` or `zig` | Pick the linker instead of checking for a working `cc`. |
| `BREVI_TOOLCHAIN_DIR` | Where it is kept (default `~/.cache/brevi/pinned`; on Windows `%LOCALAPPDATA%\brevi\toolchain`). About 1 GB unpacked; delete the folder to remove it. |

## Release downloads

Every [GitHub release](__GITHUB__/releases) carries these archives, each with the `brevi` binary, `LICENSE`, `README.md` and `BREVI.md`:

| Platform | Archive |
| --- | --- |
| Linux x86-64 | `brevi-x86_64-unknown-linux-gnu.tar.gz` |
| Linux ARM64 | `brevi-aarch64-unknown-linux-gnu.tar.gz` |
| macOS, Apple silicon | `brevi-aarch64-apple-darwin.tar.gz` |
| macOS, Intel | `brevi-x86_64-apple-darwin.tar.gz` |
| Windows x86-64 | `brevi-x86_64-pc-windows-msvc.zip` |
| Windows ARM64 (from v0.28.1) | `brevi-aarch64-pc-windows-msvc.zip` |

Each release also carries `SHA256SUMS`, with the SHA-256 of every archive. The install script checks its download against it.

## Docker

The repository's `Dockerfile` bundles `brevi` with the Rust toolchain:

```
docker build -t brevi .
docker run --rm -i -v "$PWD:/w" -w /w brevi run main.py < input.txt
docker run --rm -i brevi mcp            # the MCP server over stdio
```

## From source

```
git clone __GITHUB__
cd brevi
cargo build --release
./target/release/brevi run examples/shapes.bv
```

Or `cargo install --git __GITHUB__ brevi`. See [Contributing](/docs/contributing) for the test suite.

## Package managers

Each release is published to these registries with the same prebuilt binaries as the GitHub release. They fetch the pinned toolchain like any other install where brevi does (Linux x86-64 and macOS from v0.28.2); elsewhere they need the Rust toolchain on the PATH ([rustup.rs](https://rustup.rs)).

| Channel | Command | Notes |
| --- | --- | --- |
| PyPI | `pip install brevi`, or `uvx brevi mcp` without installing | Wheels for Linux (glibc 2.34 or later, x86-64 and ARM64), macOS 11+ (Apple silicon) and 10.12+ (Intel), and Windows x86-64 and ARM64 (ARM64 from v0.28.1). |
| npm | `npm install -g brevi`, or `npx brevi mcp` | The first run downloads the matching release binary into `~/.cache/brevi/VERSION`. Needs Node 18 or later. |
| crates.io | `cargo install brevi` | Builds from source, from v0.26.0 on. |
| MCP registry | `io.github.ets-public/brevi` | Lists the PyPI and npm packages, running `brevi mcp`. |
| Homebrew | `brew install ets-public/brevi/brevi` | Coming: the formula is ready in `packaging/homebrew`. |

## Where Brevi keeps things

Brevi caches built programs and its runtime libraries in `~/.cache/brevi` (`$BREVI_CACHE` moves it, `BREVI_CACHE_MB` caps it, default 1024 MB), so checking or running an unchanged program again replays the last result in about 10 ms. The very first build, and the first after a Rust update, also builds Brevi's runtime once, which takes about 5 to 15 seconds; after that a new small program builds in well under a second. Deleting the folder is always safe.

## Next

[Using Brevi](/docs/usage) covers the commands. To connect a coding agent, go to [For AI agents](/docs/agents).
