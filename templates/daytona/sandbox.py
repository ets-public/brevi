"""Daytona (https://daytona.io): each agent run gets a sandbox made from the `brevi` snapshot.

    pip install daytona
    export DAYTONA_API_KEY=...
    python templates/daytona/build_snapshot.py   # once: builds the `brevi` snapshot from templates/image
"""
import os, shlex, sys

from daytona import CreateSandboxFromSnapshotParams, Daytona

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from brevi_tool import Box, Grants, Result, WORKDIR  # noqa: E402

SNAPSHOT = os.environ.get("BREVI_DAYTONA_SNAPSHOT", "brevi")


class Sandbox(Box):
    def __init__(self, grants=None):
        self.grants = grants or Grants()
        # No network unless the program is granted it; Brevi then limits it to the granted hosts.
        # ephemeral: the sandbox is deleted as soon as it stops.
        self.sbx = Daytona().create(CreateSandboxFromSnapshotParams(
            snapshot=SNAPSHOT, network_block_all=not self.grants.needs_network(), ephemeral=True,
            auto_stop_interval=15))

    def write(self, path, text):
        self.sbx.fs.upload_file(text.encode(), path)

    def exec(self, cmd, timeout):
        # Daytona's exec returns one output stream, so stderr is sent to a file and read back.
        r = self.sbx.process.exec(f"({cmd}) 2> .stderr", cwd=WORKDIR, timeout=int(timeout))
        err = self.sbx.process.exec("cat .stderr", cwd=WORKDIR).result
        return Result(r.exit_code, r.result, err)

    def close(self):
        self.sbx.delete()
