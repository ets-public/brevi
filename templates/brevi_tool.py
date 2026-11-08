"""The piece every platform template shares: a `run_python` tool for an agent, run by Brevi inside a sandbox.

The platform (E2B, Daytona, Modal, Cloudflare, or Docker on your own machine) gives the code a throwaway
machine. Brevi adds what a VM cannot see: the program is compiled to a native binary that may only use the
capabilities the caller grants (`--sandbox`, `--allow`, `--allow-net`), and it is stopped at the CPU, memory,
time, output and file-size limits the caller sets (`--limit`). The model never chooses either: grants and
limits come from your code (`Grants` below), not from the tool call.

Each platform file defines a `Sandbox(Box)` with `write(path, text)`, `exec(cmd)` and `close()`; everything
else is here. See README.md.
"""
from dataclasses import dataclass, field
import json, shlex

WORKDIR = "/workspace"

# What the model sees. There is no field for permissions or limits on purpose.
TOOL = {
    "name": "run_python",
    "description": (
        "Run a Python 3 program and get its output. It is compiled to native code, so write the plain, direct "
        "solution; do not hand-optimise it for speed. It reads `stdin` and prints to stdout. Files, the network "
        "and other processes are only available if the operator granted them; an error says what was refused."
    ),
    "input_schema": {
        "type": "object",
        "properties": {
            "code": {"type": "string", "description": "The whole program (main.py)."},
            "stdin": {"type": "string", "description": "Text passed to the program on stdin."},
        },
        "required": ["code"],
        "additionalProperties": False,
    },
}


@dataclass
class Grants:
    """What the caller allows. The defaults allow nothing and stop a program after 10 s of CPU, 512 MB of
    memory, 20 s of wall time, 256 KB of output or a 16 MB file."""
    allow: list = field(default_factory=list)        # any of "read", "write", "net", "native"
    allow_net: list = field(default_factory=list)    # hosts the program may connect to, e.g. ["api.github.com"]
    allow_py: list = field(default_factory=list)     # Python modules CPython may run (needs "native")
    allow_read: list = field(default_factory=list)   # folders the program may read inside, e.g. ["data"]
    allow_write: list = field(default_factory=list)  # folders the program may write inside, e.g. ["out"]
    limits: dict = field(default_factory=lambda: {"cpu": 10, "mem": 512, "time": 20, "output": 256, "file": 16})

    def flags(self):
        f = ["--sandbox"]
        if self.allow:
            f += ["--allow", ",".join(self.allow)]
        if self.allow_net:
            f += ["--allow-net", ",".join(self.allow_net)]
        if self.allow_py:
            f += ["--allow-py", ",".join(self.allow_py)]
        if self.allow_read:
            f += ["--allow-read", ",".join(self.allow_read)]
        if self.allow_write:
            f += ["--allow-write", ",".join(self.allow_write)]
        if self.limits:
            f += ["--limit", ",".join(f"{k}={v}" for k, v in self.limits.items())]
        return f

    def needs_network(self):
        return bool(self.allow_net) or "net" in self.allow


def command(grants, main="main.py", stdin="stdin.txt"):
    """The shell command a sandbox runs: `brevi run --sandbox ... main.py < stdin.txt`."""
    return " ".join(shlex.quote(a) for a in ["brevi", "run", *grants.flags(), main]) + f" < {shlex.quote(stdin)}"


def seconds(v):
    """A --limit time value (20, "20s", "500ms") in seconds."""
    v = str(v)
    return float(v[:-2]) / 1000 if v.endswith("ms") else float(v.rstrip("s"))


@dataclass
class Result:
    exit_code: int
    stdout: str
    stderr: str

    @property
    def limit(self):
        """The limit that stopped the program ("cpu", "mem", "time", "output" or "file"), or None."""
        for line in self.stderr.splitlines():
            if line.startswith('{"e":"limit"'):
                return json.loads(line).get("on")
        return None

    def for_model(self, cap=8000):
        """What the agent reads back: the exit code, stdout and stderr, cut to `cap` characters each."""
        cut = lambda s: s if len(s) <= cap else s[:cap] + f"\n[... {len(s) - cap} more characters]"
        parts = [f"exit code {self.exit_code}"]
        if self.stdout:
            parts.append("stdout:\n" + cut(self.stdout))
        if self.stderr:
            parts.append("stderr:\n" + cut(self.stderr))
        return "\n".join(parts)


class Box:
    """A platform sandbox. Subclasses implement write, exec and close."""
    grants = Grants()

    def write(self, path, text):
        raise NotImplementedError

    def exec(self, cmd, timeout):
        """Run `sh -c cmd` in WORKDIR; return Result."""
        raise NotImplementedError

    def close(self):
        pass

    def run(self, code, stdin=""):
        """Compile and run one program under the caller's grants and limits."""
        self.write(f"{WORKDIR}/main.py", code)
        self.write(f"{WORKDIR}/stdin.txt", stdin)
        # The wall-clock limit holds the program; the platform's own timeout also covers the build.
        wall = seconds(self.grants.limits.get("time", 60)) + 120
        return self.exec(command(self.grants), timeout=wall)

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()
