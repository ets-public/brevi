"""E2B (https://e2b.dev): each agent run gets a Firecracker microVM made from the `brevi` template.

    pip install e2b
    export E2B_API_KEY=...
    python templates/e2b/build_template.py      # once: builds the `brevi` template from templates/image
"""
import os, sys

from e2b import CommandExitException, Sandbox as E2BSandbox

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from brevi_tool import Box, Grants, Result, WORKDIR  # noqa: E402

TEMPLATE = os.environ.get("BREVI_E2B_TEMPLATE", "brevi")


class Sandbox(Box):
    def __init__(self, grants=None, timeout=600):
        self.grants = grants or Grants()
        # The VM gets the internet only when the program is granted the network; Brevi then limits it to
        # the granted hosts.
        self.sbx = E2BSandbox.create(TEMPLATE, timeout=timeout, allow_internet_access=self.grants.needs_network())

    def write(self, path, text):
        self.sbx.files.write(path, text)

    def exec(self, cmd, timeout):
        try:
            r = self.sbx.commands.run(cmd, cwd=WORKDIR, timeout=timeout)
            return Result(r.exit_code, r.stdout, r.stderr)
        except CommandExitException as e:  # E2B raises on a non-zero exit
            return Result(e.exit_code, e.stdout, e.stderr)

    def close(self):
        self.sbx.kill()
