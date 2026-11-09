"""Docker on your own machine: the same image the platforms run, with no account. Good for trying the
templates and for CI. Build the image first: `docker build -t brevi-sandbox templates/image`, or (from v0.27.0)
use the published one: BREVI_IMAGE=ghcr.io/ets-public/brevi-sandbox:latest."""
import os, subprocess, sys, uuid

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from brevi_tool import Box, Grants, Result, WORKDIR  # noqa: E402

IMAGE = os.environ.get("BREVI_IMAGE", "brevi-sandbox")


class Sandbox(Box):
    def __init__(self, grants=None):
        self.grants = grants or Grants()
        self.name = f"brevi-{uuid.uuid4().hex[:12]}"
        net = [] if self.grants.needs_network() else ["--network", "none"]
        # One container per sandbox, kept alive between runs like a platform sandbox; a non-root user,
        # as on E2B and Daytona.
        subprocess.run(["docker", "run", "-d", "--rm", "--name", self.name, "--user", "1000:1000", *net,
                        "--memory", "2g", IMAGE, "sleep", "infinity"], check=True, capture_output=True)

    def write(self, path, text):
        subprocess.run(["docker", "exec", "-i", self.name, "sh", "-c", f"cat > {path}"],
                       input=text.encode(), check=True)

    def exec(self, cmd, timeout):
        try:
            r = subprocess.run(["docker", "exec", "-w", WORKDIR, self.name, "sh", "-c", cmd],
                               capture_output=True, text=True, timeout=timeout)
            return Result(r.returncode, r.stdout, r.stderr)
        except subprocess.TimeoutExpired:
            return Result(124, "", f"the sandbox gave up after {timeout:.0f} s")

    def close(self):
        subprocess.run(["docker", "rm", "-f", self.name], capture_output=True)
