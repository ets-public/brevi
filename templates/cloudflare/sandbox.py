"""Cloudflare (https://developers.cloudflare.com/sandbox/): the agent calls the Worker in this folder, which
runs each program in a Cloudflare Sandbox container. Deploy it once:

    cd templates/cloudflare && npm install && npx wrangler secret put BREVI_TOKEN && npx wrangler deploy
    export BREVI_WORKER_URL=https://brevi-sandbox.<you>.workers.dev BREVI_TOKEN=...

Grants and limits live on the Worker, in wrangler.jsonc (`vars`), so a client cannot widen them.
"""
import json, os, sys, urllib.request, uuid

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from brevi_tool import Box, Grants, Result  # noqa: E402


class Sandbox(Box):
    def __init__(self, grants=None, url=None, token=None):
        if grants is not None and grants != Grants():
            raise ValueError("set grants and limits in wrangler.jsonc on the Worker, not here")
        self.url = (url or os.environ["BREVI_WORKER_URL"]).rstrip("/")
        self.token = token or os.environ["BREVI_TOKEN"]
        self.session = f"agent-{uuid.uuid4().hex[:12]}"

    def _call(self, method, path, body=None, timeout=180):
        req = urllib.request.Request(self.url + path, method=method,
                                     data=None if body is None else json.dumps(body).encode(),
                                     headers={"Authorization": f"Bearer {self.token}", "Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return json.loads(r.read() or b"null")

    def run(self, code, stdin=""):
        # The Worker writes the files and builds the command itself.
        r = self._call("POST", "/run", {"code": code, "stdin": stdin, "session": self.session})
        return Result(r["exitCode"], r["stdout"], r["stderr"])

    def close(self):
        try:
            self._call("DELETE", f"/session/{self.session}")
        except OSError:
            pass
