"""Checks `brevi mcp --http`: Streamable HTTP in JSON-response mode over the stdio server. Requests get their
reply as JSON, notifications 202; bad bodies, media types, methods, paths and origins are refused; the bearer
token is checked; two clients using the same ids get their own replies; cancelling ends the waiting POST; a
non-local address needs a token. With the official mcp client too, where python3 has it.
usage: python3 tests/mcp_http_test.py [BREVI]"""
import json, socket, subprocess, sys, threading, time, urllib.error, urllib.request

B = sys.argv[1] if len(sys.argv) > 1 else "target/release/brevi"
fails = 0


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def check(label, got, want):
    global fails
    if got != want:
        fails += 1
        print(f"FAIL {label}: want {want!r}, got {got!r}")


def server(*extra):
    port = free_port()
    p = subprocess.Popen([B, "mcp", "--http", str(port), *extra], stderr=subprocess.PIPE, text=True)
    line = p.stderr.readline()
    check("start line", "Streamable HTTP on http://127.0.0.1:%d/mcp" % port in line, True)
    return p, "http://127.0.0.1:%d/mcp" % port


def post(url, msg=None, headers=None, raw=None, method="POST"):
    h = {"Content-Type": "application/json", "Accept": "application/json, text/event-stream"}
    h.update(headers or {})
    data = raw if raw is not None else (json.dumps(msg).encode() if msg is not None else None)
    try:
        with urllib.request.urlopen(urllib.request.Request(url, data=data, headers=h, method=method), timeout=120) as f:
            return f.status, f.read().decode()
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()


def req(id, method, params=None):
    m = {"jsonrpc": "2.0", "id": id, "method": method}
    if params is not None:
        m["params"] = params
    return m


p, U = server("--timeout", "60")
s, b = post(U, req(1, "initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}))
check("initialize", (s, json.loads(b)["result"]["protocolVersion"], json.loads(b)["id"]), (200, "2025-06-18", 1))
check("notification", post(U, {"jsonrpc": "2.0", "method": "notifications/initialized"}), (202, ""))
s, b = post(U, req("a", "tools/list"))
check("tools/list", (s, json.loads(b)["id"], len(json.loads(b)["result"]["tools"])), (200, "a", 8))
s, b = post(U, req(7, "tools/call", {"name": "run", "arguments": {"code": "print(6 * 7)"}}))
check("tools/call", (s, json.loads(b)["id"], json.loads(b)["result"]["content"][0]["text"].split()[0]), (200, 7, "42"))
check("bad JSON", post(U, raw=b"{bad")[0], 400)
check("bad JSON code", json.loads(post(U, raw=b"{bad")[1])["error"]["code"], -32700)
check("batch", post(U, raw=b"[1]")[0], 400)
check("content type", post(U, req(2, "ping"), headers={"Content-Type": "text/plain"})[0], 415)
check("accept", post(U, req(2, "ping"), headers={"Accept": "text/html"})[0], 406)
check("GET", post(U, method="GET")[0], 405)
check("DELETE", post(U, method="DELETE")[0], 405)
check("path", post(U.replace("/mcp", "/other"), req(2, "ping"))[0], 404)
check("foreign origin", post(U, req(2, "ping"), headers={"Origin": "http://evil.example"})[0], 403)
check("local origin", post(U, req(2, "ping"), headers={"Origin": "http://localhost:3000"}), (200, '{"jsonrpc":"2.0","id":2,"result":{}}'))
out = {}


def run(tag, code):
    out[tag] = post(U, req(1, "tools/call", {"name": "run", "arguments": {"code": code}}))


ts = [threading.Thread(target=run, args=(t, "print(%r)" % t)) for t in ("x", "y")]
[t.start() for t in ts]
[t.join() for t in ts]
check("same ids", [(k, v[0], json.loads(v[1])["id"], json.loads(v[1])["result"]["content"][0]["text"].split()[0]) for k, v in sorted(out.items())], [("x", 200, 1, "x"), ("y", 200, 1, "y")])
t = threading.Thread(target=run, args=("slow", "import time\ntime.sleep(30)"))
t0 = time.time()
t.start()
time.sleep(3)
check("cancel", post(U, {"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 1}}), (202, ""))
t.join(20)
check("cancelled POST ends", (out.get("slow"), time.time() - t0 < 20), ((202, ""), True))
p.terminate()
p.wait()

TOK = "a-token-of-twenty-chars"
p, U = server("--token", TOK)
check("no token", post(U, req(1, "ping"))[0], 401)
check("wrong token", post(U, req(1, "ping"), headers={"Authorization": "Bearer nope"})[0], 401)
check("token", post(U, req(1, "ping"), headers={"Authorization": "Bearer " + TOK})[0], 200)
try:
    import asyncio
    import httpx2
    from mcp import ClientSession
    from mcp.client.streamable_http import streamable_http_client
except ImportError:
    print("skip official mcp client (no mcp in python3)")
else:
    async def official():
        async with httpx2.AsyncClient(headers={"Authorization": "Bearer " + TOK}, timeout=120) as hc:
            async with streamable_http_client(U, http_client=hc) as (r, w, *_):
                async with ClientSession(r, w) as s:
                    info = await s.initialize()
                    tools = await s.list_tools()
                    res = await s.call_tool("run", {"code": "print(6 * 7)"})
                    return info.server_info.name, len(tools.tools), res.content[0].text.split()[0], res.is_error
    check("official client", asyncio.run(official()), ("brevi", 8, "42", False))
p.terminate()
p.wait()

q = subprocess.run([B, "mcp", "--http", "0.0.0.0:%d" % free_port()], capture_output=True, text=True)
check("non-local needs a token", (q.returncode, "needs a bearer token" in q.stderr), (2, True))
q = subprocess.run([B, "mcp", "--http", "0.0.0.0:%d" % free_port(), "--token", "short"], capture_output=True, text=True)
check("short token", (q.returncode, "at least 16" in q.stderr), (2, True))
print("mcp http: %d failed" % fails if fails else "mcp http: all passed")
sys.exit(1 if fails else 0)
