"""Turns the release archives into Python wheels for PyPI, with no extra build.

usage: make_wheels.py DIST_DIR OUT_DIR

DIST_DIR holds the brevi-<target>.tar.gz / .zip archives the release workflow built. Each one becomes a
wheel that puts the same `brevi` binary on the PATH (`pip install brevi`, `uvx brevi mcp`). Building the
wheels from these archives instead of with maturin saves a second macOS build (10x Actions minutes).
The manylinux tag comes from the newest glibc symbol the binary actually uses.
"""
import base64, hashlib, io, os, re, sys, tarfile, zipfile

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
# Rust's default minimum macOS versions: 11.0 on Apple silicon, 10.12 on Intel.
PLATFORMS = {
    "x86_64-unknown-linux-gnu": "manylinux_{glibc}_x86_64",
    "aarch64-unknown-linux-gnu": "manylinux_{glibc}_aarch64",
    "aarch64-apple-darwin": "macosx_11_0_arm64",
    "x86_64-apple-darwin": "macosx_10_12_x86_64",
    "x86_64-pc-windows-msvc": "win_amd64",
}


def version():
    return re.search(r'^version = "([^"]+)"', open(os.path.join(ROOT, "Cargo.toml")).read(), re.M)[1]


def metadata(ver):
    readme = open(os.path.join(ROOT, "packaging/pypi/README.md"), encoding="utf-8").read()
    head = [
        "Metadata-Version: 2.1",
        "Name: brevi",
        f"Version: {ver}",
        "Summary: Compiles the plain Python AI agents write into fast native binaries via Rust, with one-line JSON errors and an MCP server.",
        "Keywords: python,compiler,rust,mcp,ai-agents,llm,native,brevi",
        "License: Apache-2.0",
        "Project-URL: Homepage, https://brevi-lang.org",
        "Project-URL: Documentation, https://brevi-lang.org/docs/",
        "Project-URL: Source, https://github.com/ets-public/brevi",
        "Project-URL: Issues, https://github.com/ets-public/brevi/issues",
        "Project-URL: Changelog, https://github.com/ets-public/brevi/blob/main/CHANGELOG.md",
        "Classifier: Development Status :: 4 - Beta",
        "Classifier: Environment :: Console",
        "Classifier: Intended Audience :: Developers",
        "Classifier: License :: OSI Approved :: Apache Software License",
        "Classifier: Operating System :: MacOS",
        "Classifier: Operating System :: Microsoft :: Windows",
        "Classifier: Operating System :: POSIX :: Linux",
        "Classifier: Programming Language :: Python :: 3",
        "Classifier: Programming Language :: Rust",
        "Classifier: Topic :: Software Development :: Compilers",
        "Requires-Python: >=3.8",
        "Description-Content-Type: text/markdown",
    ]
    return "\n".join(head) + "\n\n" + readme


def read_binary(path):
    """(file name, bytes) of the brevi executable inside a release archive."""
    if path.endswith(".zip"):
        with zipfile.ZipFile(path) as z:
            name = next(n for n in z.namelist() if n.endswith("/brevi.exe"))
            return "brevi.exe", z.read(name)
    with tarfile.open(path) as t:
        member = next(m for m in t.getmembers() if m.name.endswith("/brevi") and m.isfile())
        return "brevi", t.extractfile(member).read()


def glibc(data):
    found = {tuple(map(int, m.split(b"."))) for m in re.findall(rb"GLIBC_(2\.\d+)", data)}
    major, minor = max(found) if found else (2, 17)
    return f"{major}_{max(minor, 17)}"


def record_line(name, data):
    digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=").decode()
    return f"{name},sha256={digest},{len(data)}"


def build(archive, target, ver, out):
    exe, data = read_binary(archive)
    plat = PLATFORMS[target].format(glibc=glibc(data))
    tag = f"py3-none-{plat}"
    info = f"brevi-{ver}.dist-info"
    files = [
        (f"brevi-{ver}.data/scripts/{exe}", data, 0o755),
        (f"{info}/METADATA", metadata(ver).encode(), 0o644),
        (f"{info}/WHEEL", f"Wheel-Version: 1.0\nGenerator: brevi make_wheels.py\nRoot-Is-Purelib: false\nTag: {tag}\n".encode(), 0o644),
        (f"{info}/licenses/LICENSE", open(os.path.join(ROOT, "LICENSE"), "rb").read(), 0o644),
    ]
    record = "\n".join(record_line(n, d) for n, d, _ in files) + f"\n{info}/RECORD,,\n"
    files.append((f"{info}/RECORD", record.encode(), 0o644))
    path = os.path.join(out, f"brevi-{ver}-{tag}.whl")
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        for name, body, mode in files:
            zi = zipfile.ZipInfo(name, date_time=(2020, 1, 1, 0, 0, 0))
            zi.external_attr = (0o100000 | mode) << 16
            zi.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(zi, body)
    return path


def main():
    dist, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    ver = version()
    made = []
    for target in PLATFORMS:
        for ext in (".tar.gz", ".zip"):
            archive = os.path.join(dist, f"brevi-{target}{ext}")
            if os.path.exists(archive):
                made.append(build(archive, target, ver, out))
    for p in made:
        print(os.path.basename(p))
    missing = len(PLATFORMS) - len(made)
    if missing:
        print(f"{missing} platform archive(s) missing from {dist}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
