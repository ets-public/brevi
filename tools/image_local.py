"""Prints templates/image/Dockerfile with its install step (Brevi and the SDK downloaded from a release) replaced by
a copy of a local binary `brevi` and SDK `sdk_init.py` from the build context, so release.yml can build and check
the image from its own build before anything is published. Every other line is the shipped file's own.
usage: python3 tools/image_local.py templates/image/Dockerfile > CTX/Dockerfile"""
import sys

lines = open(sys.argv[1]).read().split("\n")
start = next(i for i, l in enumerate(lines) if l.startswith('RUN if [ -n "$BREVI_VERSION" ]'))
end = start
while lines[end].endswith("\\"):
    end += 1
step = "\n".join(lines[start:end + 1])
# Refuse to guess if the install step changed shape: these are what the replacement stands in for.
for want in ("install.sh", "/usr/local/bin/brevi", "brevi-python", "packaging/pypi/brevi/__init__.py", "import brevi"):
    if want not in step:
        sys.exit(f"image_local: the install step no longer has {want!r}; update tools/image_local.py")
local = """COPY brevi /usr/local/bin/brevi
COPY sdk_init.py /tmp/sdk_init.py
RUN chmod 755 /usr/local/bin/brevi && ln -sf brevi /usr/local/bin/brevi-python && brevi --version \\
 && sdk="$(python3 -c 'import sysconfig; print(sysconfig.get_path("purelib"))')/brevi" && mkdir -p "$sdk" \\
 && install -m 644 /tmp/sdk_init.py "$sdk/__init__.py" && rm /tmp/sdk_init.py && python3 -c 'import brevi'"""
print("\n".join(lines[:start] + [local] + lines[end + 1:]))
