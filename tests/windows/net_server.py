# The HTTP/HTTPS server the Windows checks' requests program talks to:
#   python tests/windows/net_server.py PORT CERTDIR
# serves HTTP on 127.0.0.1:PORT and HTTPS on 127.0.0.1:PORT+1 (CERTDIR/cert.pem, CERTDIR/key.pem: the
# tests/http certificate for localhost and 127.0.0.1). Every response is the same on every run (no Date
# header), so the Brevi and CPython runs print the same bytes. Small on purpose: the Windows checks start it,
# run one requests program against it and stop it.
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
    wbufsize = 1 << 16

    def log_message(self, *a):
        pass

    def send(self, status, body=b"", ctype="application/json", headers=()):
        self.send_response_only(status)
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

    def js(self, obj, status=200):
        self.send(status, (json.dumps(obj, sort_keys=True) + "\n").encode())

    def do_GET(self):
        u = urlsplit(self.path)
        if u.path == "/echo":
            self.js({"path": u.path, "query": u.query, "method": self.command})
        elif u.path == "/text":
            self.send(200, b"hello brevi\n", "text/plain")
        elif u.path == "/json":
            self.js({"ok": True, "n": 42})
        elif u.path == "/gzip":
            body = gzip.compress(b'{"zipped": true}\n', mtime=0)
            self.send(200, body, "application/json", [("Content-Encoding", "gzip")])
        elif u.path == "/deflate":
            body = zlib.compress(b'{"deflated": true}\n')
            self.send(200, body, "application/json", [("Content-Encoding", "deflate")])
        elif u.path == "/slow":
            d = float((parse_qs(u.query).get("d") or ["1"])[0])
            time.sleep(d)
            self.send(200, b"slow\n", "text/plain")
        else:
            self.js({"error": "not found", "path": u.path}, 404)

    do_HEAD = do_GET


class S(http.server.ThreadingHTTPServer):
    allow_reuse_address = True


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 18180
    here = sys.argv[2] if len(sys.argv) > 2 else "tests/http"
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
