# The HTTP and HTTPS server the tests/http programs talk to: `python3 tests/http/server.py PORT` serves HTTP on
# 127.0.0.1:PORT and HTTPS (tests/http/cert.pem, for localhost and 127.0.0.1) on PORT+1. Every response is the
# same on every run (no Date header), so a program's output can be compared with the .expected file CPython's
# requests and httpx printed.
import base64
import gzip
import http.server
import json
import ssl
import sys
import threading
import time
import zlib
from urllib.parse import parse_qs, urlsplit


class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    # one write per response, so the client never waits on a delayed ACK between the head and the body
    wbufsize = 1 << 16

    def log_message(self, *a):
        pass

    def send(self, status, body=b"", ctype="application/json", headers=(), reason=None):
        self.send_response_only(status, reason)
        self.send_header("Server", "brevi-test")
        if ctype:
            self.send_header("Content-Type", ctype)
        names = [h[0].lower() for h in headers]
        if "transfer-encoding" not in names and "x-no-length" not in names:
            self.send_header("Content-Length", str(len(body)))
        for k, v in headers:
            if k.lower() != "x-no-length":
                self.send_header(k, v)
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)
        self.wfile.flush()

    def js(self, obj, status=200, headers=()):
        self.send(status, (json.dumps(obj, sort_keys=True) + "\n").encode(), headers=headers)

    def body(self):
        if self.headers.get("Transfer-Encoding", "").lower() == "chunked":
            out = b""
            while True:
                n = int(self.rfile.readline().strip(), 16)
                if n == 0:
                    self.rfile.readline()
                    return out
                out += self.rfile.read(n)
                self.rfile.readline()
        n = int(self.headers.get("Content-Length") or 0)
        return self.rfile.read(n) if n else b""

    def echo(self):
        u = urlsplit(self.path)
        b = self.body()
        return {
            "method": self.command,
            "path": u.path,
            "query": u.query,
            "headers": [[k, v] for k, v in self.headers.items()],
            "body": b.decode("utf-8", "replace"),
        }

    def handle_one(self):
        u = urlsplit(self.path)
        q = {k: v[-1] for k, v in parse_qs(u.query).items()}
        p = u.path
        if p in ("/echo", "/anything") or p.startswith("/anything/"):
            return self.js(self.echo())
        if p.startswith("/status/"):
            code = int(p.split("/")[2])
            self.body()
            if code in (301, 302, 303, 307, 308):
                return self.send(code, b"", None, [("Location", "/echo")])
            return self.send(code, b"status %d\n" % code, "text/plain")
        if p.startswith("/redirect/"):
            n = int(p.split("/")[2])
            self.body()
            loc = "/echo" if n <= 1 else "/redirect/%d" % (n - 1)
            return self.send(302, b"", None, [("Location", loc)])
        if p == "/redirect-to":
            self.body()
            return self.send(int(q.get("status", "302")), b"", None, [("Location", q["url"])])
        if p == "/absolute-redirect":
            self.body()
            return self.send(301, b"", None, [("Location", "http://127.0.0.1:%d/echo?from=abs" % self.server.server_port)])
        if p == "/utf8":
            return self.send(200, "héllo wörld ☃\n".encode(), "text/plain; charset=utf-8")
        if p == "/latin1":
            return self.send(200, "café\n".encode("latin-1"), "text/plain; charset=ISO-8859-1")
        if p == "/text-nocharset":
            return self.send(200, "naïve text\n".encode(), "text/plain")
        if p == "/html":
            return self.send(200, b"<html><body>hi</body></html>", "text/html")
        if p == "/json-nocharset":
            return self.send(200, json.dumps({"name": "brevi", "ok": True, "n": [1, 2.5, None]}).encode(), "application/json")
        if p == "/json-utf16":
            return self.send(200, json.dumps({"x": "é"}).encode("utf-16-le"), "application/json")
        if p == "/bad-json":
            return self.send(200, b"not json at all", "application/json")
        if p == "/empty":
            return self.send(204, b"", None)
        if p == "/gzip":
            return self.send(200, gzip.compress(b'{"gzipped": true}\n' * 3, mtime=0), "application/json", [("Content-Encoding", "gzip")])
        if p == "/deflate":
            return self.send(200, zlib.compress(b'{"deflated": true}\n'), "application/json", [("Content-Encoding", "deflate")])
        if p == "/raw-deflate":
            c = zlib.compressobj(wbits=-15)
            return self.send(200, c.compress(b"raw deflate body\n" * 4) + c.flush(), "text/plain", [("Content-Encoding", "deflate")])
        if p == "/big":
            data = bytes((i * 7 + i // 256) % 251 for i in range(300000))
            return self.send(200, gzip.compress(data, mtime=0) if q.get("gz") else data, "application/octet-stream", [("Content-Encoding", "gzip")] if q.get("gz") else [])
        if p == "/chunked":
            self.send_response_only(200)
            self.send_header("Server", "brevi-test")
            self.send_header("Content-Type", "text/plain")
            self.send_header("Transfer-Encoding", "chunked")
            self.end_headers()
            for part in [b"first line\n", b"second ", b"line\nthird line\n", b"no newline at end"]:
                self.wfile.write(b"%x\r\n%s\r\n" % (len(part), part))
                self.wfile.flush()
                time.sleep(0.02)
            self.wfile.write(b"0\r\n\r\n")
            return
        if p == "/sse":
            self.send_response_only(200)
            self.send_header("Server", "brevi-test")
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Transfer-Encoding", "chunked")
            self.end_headers()
            for i in range(int(q.get("n", "3"))):
                ev = ('data: {"i": %d, "text": "tok%d"}\n\n' % (i, i)).encode()
                self.wfile.write(b"%x\r\n%s\r\n" % (len(ev), ev))
                self.wfile.flush()
                time.sleep(float(q.get("gap", "0.01")))
            done = b"data: [DONE]\n\n"
            self.wfile.write(b"%x\r\n%s\r\n0\r\n\r\n" % (len(done), done))
            return
        if p == "/close":
            # no Content-Length: the body ends when the server closes the connection
            self.close_connection = True
            self.send_response_only(200)
            self.send_header("Server", "brevi-test")
            self.send_header("Content-Type", "text/plain")
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(b"body until close\n")
            return
        if p == "/slow":
            time.sleep(float(q.get("d", "1")))
            return self.js({"slept": float(q.get("d", "1"))})
        if p == "/cookies/set":
            hs = [("Set-Cookie", "%s=%s; Path=/" % (k, v)) for k, v in sorted(q.items())]
            return self.send(302, b"", None, hs + [("Location", "/cookies")])
        if p == "/cookies":
            return self.js({"cookie": self.headers.get("Cookie")})
        if p.startswith("/basic-auth/"):
            _, _, user, pw = p.split("/")
            want = "Basic " + base64.b64encode(("%s:%s" % (user, pw)).encode()).decode()
            if self.headers.get("Authorization") == want:
                return self.js({"authenticated": True, "user": user})
            return self.send(401, b"", None, [("WWW-Authenticate", 'Basic realm="test"')])
        if p == "/headers-out":
            return self.send(200, b"ok\n", "text/plain", [("X-Multi", "a"), ("X-Multi", "b"), ("x-lower", "v"), ("Link", '<http://x/?page=2>; rel="next", <http://x/?page=9>; rel="last"')])
        if p == "/reason":
            return self.send(418, b"teapot\n", "text/plain", reason="I'm a teapot")
        if p == "/custom-reason":
            return self.send(299, b"odd\n", "text/plain", reason="Fine Thanks")
        if p == "/lines":
            return self.send(200, b"a\r\nb\n\nc\rd\n", "text/plain")
        return self.send(404, b"not found\n", "text/plain")

    def do_GET(self):
        self.handle_one()

    do_POST = do_PUT = do_PATCH = do_DELETE = do_HEAD = do_OPTIONS = do_GET


class S(http.server.ThreadingHTTPServer):
    daemon_threads = True
    request_queue_size = 256
    allow_reuse_address = True


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 18080
    here = __file__.rsplit("/", 1)[0] or "."
    plain = S(("127.0.0.1", port), H)
    tls = S(("127.0.0.1", port + 1), H)
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(here + "/cert.pem", here + "/key.pem")
    tls.socket = ctx.wrap_socket(tls.socket, server_side=True)
    threading.Thread(target=tls.serve_forever, daemon=True).start()
    print("ready", flush=True)
    plain.serve_forever()


if __name__ == "__main__":
    main()
