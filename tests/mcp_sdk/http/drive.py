# Drives an MCP server over its HTTP transports with raw HTTP/1.1 requests and prints a normalized transcript
# (status, the headers that matter, the body or its SSE events; session ids and pings blanked), so a server on
# CPython's mcp (Starlette, uvicorn) and the same server built by Brevi can be compared line for line:
#   python3 tests/mcp_sdk/http/drive.py CASES PORT -- COMMAND...
# CASES is "streamable", "json", "stateless" or "sse": which transport the COMMAND serves on PORT.
import json
import re
import socket
import subprocess
import sys
import time

cases, port = sys.argv[1], int(sys.argv[2])
cmd = sys.argv[sys.argv.index("--") + 1:]
KEEP = ("content-type", "allow", "location", "mcp-session-id")
INIT = {"jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "drive", "version": "1"}}}
ACC = "application/json, text/event-stream"
class _Out(list):
    # printed as it goes, so a run that hangs shows where
    def append(self, line):
        print(line, flush=True)


out = _Out()


def req(method, path, body=None, headers=None, raw=None, stream_first=False, timeout=20):
    s = socket.create_connection(("127.0.0.1", port), timeout=timeout)
    h = {"host": "127.0.0.1:%d" % port}
    if body is not None:
        h["content-type"] = "application/json"
    h.update(headers or {})
    data = raw if raw is not None else (b"" if body is None else (body if isinstance(body, bytes) else json.dumps(body).encode()))
    if data or method == "POST":
        h["content-length"] = str(len(data))
    lines = ["%s %s HTTP/1.1" % (method, path)] + ["%s: %s" % (k, v) for k, v in h.items() if v is not None]
    s.sendall(("\r\n".join(lines) + "\r\n\r\n").encode() + data)
    buf = b""
    while b"\r\n\r\n" not in buf:
        c = s.recv(65536)
        if not c:
            break
        buf += c
    head, _, rest = buf.partition(b"\r\n\r\n")
    hl = head.decode("latin-1").split("\r\n")
    status = int(hl[0].split(" ")[1])
    hd = []
    for ln in hl[1:]:
        k, _, v = ln.partition(":")
        hd.append((k.strip().lower(), v.strip()))
    hdict = dict(hd)
    body = b""
    if hdict.get("transfer-encoding") == "chunked":
        body = dechunk(s, rest, stream_first)
    elif "content-length" in hdict:
        n = int(hdict["content-length"])
        body = rest
        while len(body) < n:
            c = s.recv(65536)
            if not c:
                break
            body += c
    s.close()
    return status, hd, body


def dechunk(s, buf, first_only):
    out = b""
    while True:
        while b"\r\n" not in buf:
            c = s.recv(65536)
            if not c:
                return out
            buf += c
        ln, _, buf = buf.partition(b"\r\n")
        n = int(ln.split(b";")[0], 16)
        if n == 0:
            return out
        while len(buf) < n + 2:
            c = s.recv(65536)
            if not c:
                return out + buf
            buf += c
        out += buf[:n]
        buf = buf[n + 2:]
        if first_only and b"\n\n" in out.replace(b"\r\n", b"\n"):
            return out


class Stream:
    # an open SSE response: events one at a time
    def __init__(self, path, headers):
        self.s = socket.create_connection(("127.0.0.1", port), timeout=20)
        h = {"host": "127.0.0.1:%d" % port}
        h.update(headers)
        self.s.sendall(("GET %s HTTP/1.1\r\n" % path + "".join("%s: %s\r\n" % kv for kv in h.items()) + "\r\n").encode())
        buf = b""
        while b"\r\n\r\n" not in buf:
            buf += self.s.recv(65536)
        head, _, self.raw = buf.partition(b"\r\n\r\n")
        self.status = int(head.split(b" ")[1])
        self.text = b""

    def _more(self):
        # one chunk of the chunked body into text
        while b"\r\n" not in self.raw:
            c = self.s.recv(65536)
            if not c:
                return False
            self.raw += c
        ln, _, rest = self.raw.partition(b"\r\n")
        n = int(ln.split(b";")[0], 16)
        while len(rest) < n + 2:
            c = self.s.recv(65536)
            if not c:
                return False
            rest += c
        self.text += rest[:n]
        self.raw = rest[n + 2:]
        return n > 0

    def event(self):
        while True:
            t = self.text.replace(b"\r\n", b"\n")
            k = t.find(b"\n\n")
            if k >= 0:
                ev = t[:k].decode()
                self.text = t[k + 2:]
                if all(x.startswith(":") for x in ev.split("\n")):
                    continue
                return ev
            if not self._more():
                return None

    def close(self):
        self.s.close()


def req_get_head(h):
    st = Stream("/mcp", dict(h, accept="text/event-stream"))
    st.close()
    # (time for the server to see the stream close before the next request opens another)
    time.sleep(1)
    return (st.status, [], b"")


def hold_and_second(h):
    st = Stream("/mcp", dict(h, accept="text/event-stream"))
    time.sleep(0.3)
    r = req("GET", "/mcp", None, dict(h, accept="text/event-stream"))
    st.close()
    time.sleep(0.3)
    return r


SID = re.compile(r"[0-9a-f]{32}")


def norm(text):
    return SID.sub("<sid>", text).replace(":%d" % port, ":<port>")


def show(label, r, events=True):
    status, hd, body = r
    out.append("== %s" % label)
    out.append("status %d" % status)
    for k, v in hd:
        if k in KEEP:
            v = norm(v)
            if k == "content-type" and v.startswith("text/event-stream"):
                v = "text/event-stream"
            out.append("%s: %s" % (k, v))
    t = body.decode("utf-8", "replace").replace("\r\n", "\n")
    for ln in t.split("\n"):
        if ln.startswith(": ping") or not ln.strip():
            continue
        out.append("| " + norm(ln))


def sid_of(r):
    return dict(r[1]).get("mcp-session-id")


def call(i, name, args, meta=None):
    p = {"name": name, "arguments": args}
    if meta:
        p["_meta"] = meta
    return {"jsonrpc": "2.0", "id": i, "method": "tools/call", "params": p}


proc = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
try:
    t0 = time.time()
    while True:
        try:
            socket.create_connection(("127.0.0.1", port), timeout=1).close()
            break
        except OSError:
            if proc.poll() is not None or time.time() - t0 > 60:
                sys.stderr.write(proc.stderr.read().decode())
                raise SystemExit("server did not start")
            time.sleep(0.1)

    if cases in ("streamable", "json", "stateless"):
        a = {"accept": ACC}
        r = req("POST", "/mcp", INIT, a)
        show("initialize", r)
        sid = sid_of(r)
        a2 = dict(a)
        if sid:
            a2["mcp-session-id"] = sid
        a2["mcp-protocol-version"] = "2025-06-18"
        show("initialized", req("POST", "/mcp", {"jsonrpc": "2.0", "method": "notifications/initialized"}, a2))
        show("tools/list", req("POST", "/mcp", {"jsonrpc": "2.0", "id": 2, "method": "tools/list"}, a2))
        show("call add", req("POST", "/mcp", call(3, "add", {"a": 2, "b": 40}), a2))
        show("call slow", req("POST", "/mcp", call("s4", "slow", {"n": 2}, {"progressToken": "p1"}), a2))
        show("call unknown", req("POST", "/mcp", call(5, "nope", {}), a2))
        show("string id 7", req("POST", "/mcp", {"jsonrpc": "2.0", "id": "7", "method": "ping"}, a2))
        show("no session", req("POST", "/mcp", {"jsonrpc": "2.0", "id": 6, "method": "tools/list"}, {"accept": ACC}))
        show("unknown session", req("POST", "/mcp", {"jsonrpc": "2.0", "id": 6, "method": "tools/list"}, {"accept": ACC, "mcp-session-id": "abc"}))
        show("accept json only", req("POST", "/mcp", {"jsonrpc": "2.0", "id": 8, "method": "ping"}, dict(a2, accept="application/json")))
        show("accept none", req("POST", "/mcp", {"jsonrpc": "2.0", "id": 8, "method": "ping"}, dict(a2, accept="text/html")))
        show("text/plain body", req("POST", "/mcp", raw=b"{}", headers=dict(a2, **{"content-type": "text/plain"})))
        show("jsonx", req("POST", "/mcp", raw=b"{}", headers=dict(a2, **{"content-type": "application/jsonx"})))
        show("parse error", req("POST", "/mcp", raw=b"{bad", headers=dict(a2, **{"content-type": "application/json"})))
        r = req("POST", "/mcp", raw=b'{"x": 1}', headers=dict(a2, **{"content-type": "application/json"}))
        show("not json-rpc", (r[0], r[1], re.sub(rb'"message":"Validation error:[^"]*"', b'"message":"Validation error: ..."', r[2])))
        show("evil host", req("POST", "/mcp", {"jsonrpc": "2.0", "id": 9, "method": "ping"}, dict(a2, host="evil.example")))
        show("evil origin", req("POST", "/mcp", {"jsonrpc": "2.0", "id": 9, "method": "ping"}, dict(a2, origin="http://evil.example")))
        show("local origin", req("POST", "/mcp", {"jsonrpc": "2.0", "id": 9, "method": "ping"}, dict(a2, origin="http://localhost:3000")))
        show("PUT", req("PUT", "/mcp", None, a2))
        show("other path", req("GET", "/other", None, {}))
        show("trailing slash", req("POST", "/mcp/", {"jsonrpc": "2.0", "id": 9, "method": "ping"}, a2))
        show("too large", req("POST", "/mcp", raw=b" " * (4 * 1024 * 1024 + 1), headers=dict(a2, **{"content-type": "application/json"})))
        if cases == "streamable":
            show("GET stream", req_get_head(a2))
            show("GET second stream", hold_and_second(a2))
        show("GET no sse", req("GET", "/mcp", None, dict(a2, accept="application/json")))
        if cases == "stateless":
            # no session to check: a stream that stays open
            show("GET no session", req_get_head({}))
        else:
            show("GET no session", req("GET", "/mcp", None, {"accept": "text/event-stream"}))
        env = {"io.modelcontextprotocol/protocolVersion": "2026-07-28", "io.modelcontextprotocol/clientCapabilities": {}}
        mh = {"accept": ACC, "mcp-protocol-version": "2026-07-28", "mcp-method": "server/discover"}
        show("modern discover", req("POST", "/mcp", {"jsonrpc": "2.0", "id": 10, "method": "server/discover", "params": {"_meta": env}}, mh))
        mh2 = dict(mh, **{"mcp-method": "tools/call", "mcp-name": "add"})
        show("modern call", req("POST", "/mcp", {"jsonrpc": "2.0", "id": 11, "method": "tools/call", "params": {"_meta": env, "name": "add", "arguments": {"a": 1, "b": 2}}}, mh2))
        show("modern slow", req("POST", "/mcp", {"jsonrpc": "2.0", "id": 12, "method": "tools/call", "params": {"_meta": dict(env, progressToken=3), "name": "slow", "arguments": {"n": 2}}}, dict(mh2, **{"mcp-name": "slow"})))
        show("modern name mismatch", req("POST", "/mcp", {"jsonrpc": "2.0", "id": 13, "method": "tools/call", "params": {"_meta": env, "name": "add", "arguments": {"a": 1, "b": 2}}}, dict(mh2, **{"mcp-name": "slow"})))
        show("modern method mismatch", req("POST", "/mcp", {"jsonrpc": "2.0", "id": 14, "method": "tools/list", "params": {"_meta": env}}, mh))
        show("modern no meta", req("POST", "/mcp", {"jsonrpc": "2.0", "id": 15, "method": "tools/list"}, dict(mh, **{"mcp-method": "tools/list"})))
        show("modern bad version", req("POST", "/mcp", {"jsonrpc": "2.0", "id": 16, "method": "tools/list", "params": {"_meta": dict(env, **{"io.modelcontextprotocol/protocolVersion": "2099-01-01"})}}, dict(mh, **{"mcp-method": "tools/list", "mcp-protocol-version": "2099-01-01"})))
        show("modern notification", req("POST", "/mcp", {"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 1}}, mh))
        show("modern parse", req("POST", "/mcp", raw=b"{bad", headers=dict(mh, **{"content-type": "application/json"})))
        show("modern GET", req("GET", "/mcp", None, mh))
        show("modern batch", req("POST", "/mcp", [{"jsonrpc": "2.0", "id": 1, "method": "ping"}], mh))
        if sid:
            show("DELETE", req("DELETE", "/mcp", None, a2))
            show("after DELETE", req("POST", "/mcp", {"jsonrpc": "2.0", "id": 17, "method": "ping"}, a2))
            show("DELETE no session", req("DELETE", "/mcp", None, {"accept": ACC}))
    else:
        st = Stream("/sse", {"accept": "text/event-stream"})
        out.append("== GET /sse")
        out.append("status %d" % st.status)
        next_event = st.event
        ev = next_event()
        out.append("| " + norm(ev.strip().replace("\n", " / ")))
        path = ev.split("data: ")[1].strip()
        show("init POST", req("POST", path, INIT, {}))
        out.append("| " + norm(next_event().strip().replace("\n", " / ")))
        show("initialized POST", req("POST", path, {"jsonrpc": "2.0", "method": "notifications/initialized"}, {}))
        show("call POST", req("POST", path, call(2, "add", {"a": 5, "b": 6}), {}))
        out.append("| " + norm(next_event().strip().replace("\n", " / ")))
        show("slow POST", req("POST", path, call(3, "slow", {"n": 1}, {"progressToken": 9}), {}))
        for _ in range(3):
            out.append("| " + norm(next_event().strip().replace("\n", " / ")))
        show("no session_id", req("POST", "/messages/", {"jsonrpc": "2.0", "id": 4, "method": "ping"}, {}))
        show("bad session_id", req("POST", "/messages/?session_id=zz", {"jsonrpc": "2.0", "id": 4, "method": "ping"}, {}))
        show("unknown session_id", req("POST", "/messages/?session_id=" + "0" * 32, {"jsonrpc": "2.0", "id": 4, "method": "ping"}, {}))
        show("bad body", req("POST", path, raw=b'{"x":1}', headers={"content-type": "application/json"}))
        show("text body", req("POST", path, raw=b'{}', headers={"content-type": "text/plain"}))
        show("GET messages", req("GET", path, None, {}))
        show("POST /sse", req("POST", "/sse", {"jsonrpc": "2.0", "id": 4, "method": "ping"}, {}))
        show("evil host /sse", req("GET", "/sse", None, {"host": "evil.example"}))
        show("other", req("GET", "/nope", None, {}))
        st.close()
except Exception:
    import traceback
    out.append("!! driver failed:\n" + traceback.format_exc())
    proc.terminate()
    out.append("!! server stderr:\n" + proc.stderr.read().decode()[-3000:])
finally:
    proc.terminate()
    try:
        proc.wait(5)
    except subprocess.TimeoutExpired:
        proc.kill()
