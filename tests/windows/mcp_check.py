# The Windows MCP checks: a native stdio server and a native streamable-HTTP server built by Brevi's own mcp
# (never CPython) must answer an initialize, a tools/list and a tools/call over the two transports the SDK
# serves. tests/mcp_sdk/drive.py plays a stdio session in lockstep; tests/mcp_sdk/http/drive.py sends raw
# HTTP/1.1 requests and prints status, headers and bodies. Run from tests/windows/check.ps1.
#
#   python tests/windows/mcp_check.py
#
# Needs target\release\brevi.exe (run `cargo build --release` first) and rustc on PATH.
import json
import os
import socket
import subprocess
import sys
import tempfile
import time
import http.client

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
# BREVI may name the compiler (check.sh's convention); the release build is the default.
BREVI = os.environ.get("BREVI") or os.path.join(ROOT, "target", "release", "brevi.exe")
DRIVE = os.path.join(ROOT, "tests", "mcp_sdk", "drive.py")
HTTP_DRIVE = os.path.join(ROOT, "tests", "mcp_sdk", "http", "drive.py")
INIT = {"jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "win-check", "version": "1"}}}
STDIO_SESSION = [
    INIT,
    {"jsonrpc": "2.0", "method": "notifications/initialized"},
    {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
    {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "add", "arguments": {"a": 40, "b": 2}}},
]


def fail(msg):
    print("FAIL " + msg)
    sys.exit(1)


def build(py, exe):
    # the path report on stderr says the program was built over dynamic values (Brevi's own mcp), never by
    # handing the file to CPython ("py"). --no-cache: a replay from a different interpreter/path must not
    # turn a good build into "the dynamic build's code did not compile".
    env = dict(os.environ, BREVI_PATH="1")
    r = subprocess.run([BREVI, "build", "--no-cache", py, "-o", exe], capture_output=True, text=True, env=env)
    report = (r.stdout or "") + (r.stderr or "")
    if r.returncode != 0:
        fail("build %s exited %d\n%s" % (py, r.returncode, report))
    if '"path":"dynamic"' not in report or '"py"' in report:
        fail("build %s did not take the native dynamic path\n%s" % (py, report))


def run(cmd, **kw):
    # the transcripts are UTF-8 whatever the console code page is
    env = dict(os.environ, PYTHONIOENCODING="utf-8")
    try:
        return subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=240, env=env, **kw)
    except subprocess.TimeoutExpired as e:
        out = (e.stdout or "") if isinstance(e.stdout, str) else (e.stdout or b"").decode("utf-8", "replace")
        err = (e.stderr or "") if isinstance(e.stderr, str) else (e.stderr or b"").decode("utf-8", "replace")
        fail("%s timed out\n--- stdout ---\n%s\n--- stderr ---\n%s" % (" ".join(cmd), out, err))


def replies(transcript):
    # the JSON-RPC replies a drive.py transcript holds, by id
    got = {}
    for line in transcript.splitlines():
        body = None
        if line.startswith("< "):
            body = line[2:]
        elif line.startswith("| data: "):
            body = line[8:]
        elif line.startswith("| {"):
            body = line[2:]
        if body is None:
            continue
        try:
            msg = json.loads(body)
        except ValueError:
            continue
        if isinstance(msg, dict) and msg.get("id") is not None:
            got[msg["id"]] = msg
    return got


def check_session(got, name, want):
    for rid in (1, 2, 3):
        if rid not in got:
            fail("%s: no reply to request %d" % (name, rid))
    server = got[1]["result"]["serverInfo"]
    if server.get("name") != want:
        fail("%s: serverInfo.name is %r, wanted %r" % (name, server.get("name"), want))
    tools = [t["name"] for t in got[2]["result"]["tools"]]
    if tools != ["add"]:
        fail("%s: tools/list gave %r, wanted ['add']" % (name, tools))
    content = got[3]["result"]["content"]
    if content[0].get("text") != "42":
        fail("%s: tools/call add(40, 2) gave %r, wanted '42'" % (name, content))


def check_stdio(tmp):
    exe = os.path.join(tmp, "mcp_stdio.exe")
    build(os.path.join(ROOT, "tests", "windows", "mcp_stdio.py"), exe)
    session = os.path.join(tmp, "session.jsonl")
    with open(session, "w") as f:
        for msg in STDIO_SESSION:
            f.write(json.dumps(msg) + "\n")
    r = run([sys.executable, DRIVE, session, "--", exe])
    out = r.stdout
    bad = [m for m in ("! timeout", "! eof", "! broken pipe") if m in out]
    if r.returncode != 0 or bad:
        fail("stdio server: the session did not finish cleanly\n%s%s" % (out, r.stderr))
    if not out.rstrip().endswith("exit 0"):
        fail("stdio server: did not exit 0 at the end of the session\n%s" % out)
    check_session(replies(out), "stdio", "win-stdio")
    print("PASS mcp stdio (initialize, tools/list, tools/call add(40, 2) -> 42, exit 0)")


def check_http(tmp):
    exe = os.path.join(tmp, "mcp_http.exe")
    build(os.path.join(ROOT, "tests", "windows", "mcp_http.py"), exe)
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()

    # Keep the CI check to the protocol path it needs: initialize, initialized, tools/list, tools/call.
    # The full tests/mcp_sdk/http/drive.py transcript (SSE GET stream, touch tool, ...) is exercised by
    # tests/check.sh on the other platforms and by the independent Windows verification; on the GitHub
    # Windows runner its GET-stream step can wait forever, which made this check flaky.
    srv = subprocess.Popen([exe, str(port)], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    try:
        ready = False
        for _ in range(120):
            try:
                socket.create_connection(("127.0.0.1", port), timeout=1).close()
                ready = True
                break
            except OSError:
                if srv.poll() is not None:
                    break
                time.sleep(0.25)
        if not ready:
            fail("streamable-HTTP server did not listen on 127.0.0.1:%d\n%s" % (port, (srv.stderr.read() or b"").decode("utf-8", "replace")))

        def post(msg, headers):
            c = http.client.HTTPConnection("127.0.0.1", port, timeout=20)
            h = {"accept": "application/json, text/event-stream", "content-type": "application/json"}
            h.update(headers)
            c.request("POST", "/mcp", body=json.dumps(msg), headers=h)
            r = c.getresponse()
            body = r.read().decode("utf-8", "replace")
            hs = {k.lower(): v for k, v in r.getheaders()}
            c.close()
            return r.status, hs, body

        def reply(status, body, rid):
            if status != 200:
                fail("streamable-HTTP: status %d for id %s\n%s" % (status, rid, body))
            for line in body.splitlines():
                if line.startswith("data: "):
                    msg = json.loads(line[6:])
                    if msg.get("id") == rid:
                        return msg
            fail("streamable-HTTP: no reply to id %s\n%s" % (rid, body))

        status, hs, body = post(INIT, {})
        init = reply(status, body, 1)
        if init["result"]["serverInfo"].get("name") != "win-http":
            fail("streamable-HTTP: serverInfo.name is not win-http\n%s" % body)
        sid = hs.get("mcp-session-id", "")
        if not sid:
            fail("streamable-HTTP: no mcp-session-id\n%s" % hs)
        session = {"mcp-session-id": sid}
        status, _, body = post({"jsonrpc": "2.0", "method": "notifications/initialized"}, session)
        if status != 202:
            fail("streamable-HTTP: initialized gave status %d, wanted 202\n%s" % (status, body))
        status, _, body = post({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}, session)
        tools = reply(status, body, 2)["result"]["tools"]
        if [t["name"] for t in tools] != ["add"]:
            fail("streamable-HTTP: tools/list gave %r, wanted ['add']" % tools)
        status, _, body = post({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "add", "arguments": {"a": 40, "b": 2}}}, session)
        content = reply(status, body, 3)["result"]["content"]
        if content[0].get("text") != "42":
            fail("streamable-HTTP: tools/call add(40, 2) gave %r, wanted '42'\n%s" % (content, body))
    finally:
        srv.terminate()
        try:
            srv.wait(5)
        except subprocess.TimeoutExpired:
            srv.kill()
    print("PASS mcp http (initialize POST http://127.0.0.1:PORT/mcp 200, tools/list, tools/call add(40, 2) -> 42)")


def main():
    if not os.path.exists(BREVI):
        fail("brevi.exe not built; run cargo build --release first")
    tmp = tempfile.mkdtemp(prefix="brevi-mcp-win-")
    check_stdio(tmp)
    check_http(tmp)
    print("all Windows MCP checks passed")


main()
