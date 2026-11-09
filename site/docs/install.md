# Install Brevi
<!-- description: Install the Brevi compiler on Linux, macOS or Windows from the GitHub release, pip, npm, Docker, or from source. -->

Brevi turns programs into Rust and calls `rustc` to build them, so it needs the Rust toolchain on the machine as well as the `brevi` binary itself. The install script sets up both.

## Requirements

| What | Why | Notes |
| --- | --- | --- |
| `rustc` (Rust stable) | Brevi compiles every program through it | Installed by the script if missing. Any recent stable works; CI tests stable daily and beta and nightly weekly. |
| `python3` (optional) | The CPython fallback of `brevi run`, and programs that import libraries such as numpy | Not needed for programs that use only the standard library. Library programs need `libpython` where they run, unless built with `--standalone`. |
| Linux, macOS or Windows | Prebuilt binaries for x86-64 and ARM64 Linux, Apple silicon and Intel Macs, and x86-64 Windows | Library calls through CPython and `--standalone` builds are not available on Windows yet. |

## Linux and macOS: the install script

<!--public-->
```
curl -fsSL https://raw.githubusercontent.com/ets-public/brevi/main/install.sh | sh
```
<!--/public-->
<!--private-->
From a clone of the repository (it is private until the public release):

```
sh install.sh
```
<!--/private-->

The script:

1. installs the Rust toolchain with rustup (minimal profile) if `rustc` is not on the PATH;
2. downloads the prebuilt `brevi` for this machine from the latest GitHub release;
3. verifies the download against the release's `SHA256SUMS` (with `sha256sum` or `shasum -a 256`) and installs it into `~/.cargo/bin`;
4. builds it with `cargo install` instead when no release fits (set `BREVI_FROM_SOURCE=1` to always build from source);
5. prints `brevi --version`.

If the checksum does not match, or the download cannot be verified because `SHA256SUMS` or a checksum tool is missing, the script stops and installs nothing.

Check it worked:

```
$ brevi --version
brevi __FULLVERSION__
$ printf 'print(sum(range(10)))\n' > hello.py && brevi run hello.py
45
```

## Windows

1. Install Rust with [rustup](https://rustup.rs). It also asks for the Microsoft C++ build tools, which `rustc` needs to link programs.
2. Download `brevi-x86_64-pc-windows-msvc.zip` from the latest [GitHub release](__GITHUB__/releases/latest), unzip it, and put `brevi.exe` on your PATH (for example in `%USERPROFILE%\.cargo\bin`, next to `rustc`).
3. In PowerShell or cmd: `brevi --version`.

Standard-library programs build and run natively on Windows. Python files that import packages such as numpy run with CPython instead, because the library bridge does not run on Windows yet.

## Release downloads

Every [GitHub release](__GITHUB__/releases) carries these archives, each with the `brevi` binary, `LICENSE`, `README.md` and `BREVI.md`:

| Platform | Archive |
| --- | --- |
| Linux x86-64 | `brevi-x86_64-unknown-linux-gnu.tar.gz` |
| Linux ARM64 | `brevi-aarch64-unknown-linux-gnu.tar.gz` |
| macOS, Apple silicon | `brevi-aarch64-apple-darwin.tar.gz` |
| macOS, Intel | `brevi-x86_64-apple-darwin.tar.gz` |
| Windows x86-64 | `brevi-x86_64-pc-windows-msvc.zip` |

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

Each release is published to these registries with the same prebuilt binaries as the GitHub release. They need the Rust toolchain on the PATH too ([rustup.rs](https://rustup.rs)).

| Channel | Command | Notes |
| --- | --- | --- |
| PyPI | `pip install brevi`, or `uvx brevi mcp` without installing | Wheels for Linux (glibc 2.34 or later, x86-64 and ARM64), macOS 11+ (Apple silicon) and 10.12+ (Intel), and Windows x86-64. |
| npm | `npm install -g brevi`, or `npx brevi mcp` | The first run downloads the matching release binary into `~/.cache/brevi/VERSION`. Needs Node 18 or later. |
| crates.io | `cargo install brevi` | Builds from source, from v0.26.0 on. |
| MCP registry | `io.github.ets-public/brevi` | Lists the PyPI and npm packages, running `brevi mcp`. |
| Homebrew | `brew install ets-public/brevi/brevi` | Coming: the formula is ready in `packaging/homebrew`. |

## Where Brevi keeps things

Brevi caches built programs and its runtime libraries in `~/.cache/brevi` (`$BREVI_CACHE` moves it, `BREVI_CACHE_MB` caps it, default 1024 MB), so checking or running an unchanged program again replays the last result in about 10 ms. The very first build, and the first after a Rust update, also builds Brevi's runtime once, which takes about 5 to 15 seconds; after that a new small program builds in well under a second. Deleting the folder is always safe.

## Next

[Using Brevi](/docs/usage) covers the commands. To connect a coding agent, go to [For AI agents](/docs/agents).
