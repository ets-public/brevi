"""Modal (https://modal.com): each agent run gets a gVisor sandbox built from templates/image.

    pip install modal
    modal setup                                  # or MODAL_TOKEN_ID / MODAL_TOKEN_SECRET
Modal builds the image on first use and caches it; there is no separate build step.
"""
import os, sys

import modal

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from brevi_tool import Box, Grants, Result, WORKDIR  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
# BREVI_IMAGE=ghcr.io/ets-public/brevi-sandbox:latest pulls the published image (from v0.27.0) instead of building.
IMAGE = (modal.Image.from_registry(os.environ["BREVI_IMAGE"]) if os.environ.get("BREVI_IMAGE")
         else modal.Image.from_dockerfile(os.path.join(HERE, "..", "image", "Dockerfile")))


class Sandbox(Box):
    def __init__(self, grants=None, timeout=600):
        self.grants = grants or Grants()
        app = modal.App.lookup(os.environ.get("BREVI_MODAL_APP", "brevi-sandbox"), create_if_missing=True)
        if self.grants.needs_network():
            net = {}
            if self.grants.allow_net and "net" not in self.grants.allow:
                # Modal can hold the network to the granted hosts too, beside Brevi's own check.
                net = {"outbound_domain_allowlist": list(self.grants.allow_net)}
        else:
            net = {"block_network": True}
        # rustc wants memory when it links a program: 2 CPUs and 2 GB.
        self.sb = modal.Sandbox.create(app=app, image=IMAGE, timeout=timeout, cpu=2, memory=2048,
                                       workdir=WORKDIR, **net)

    def write(self, path, text):
        self.sb.filesystem.write_bytes(text.encode(), path)

    def exec(self, cmd, timeout):
        p = self.sb.exec("sh", "-c", cmd, workdir=WORKDIR, timeout=int(timeout))
        p.wait()
        return Result(p.returncode, p.stdout.read(), p.stderr.read())

    def close(self):
        self.sb.terminate()
