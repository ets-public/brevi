# HTTP/1.1 for Brevi's requests and httpx: connections (plain, TLS, through an HTTP proxy), sending a request,
# reading the response head and body (Content-Length, chunked, or until the server closes), and the
# gzip / deflate content decoders. Sockets, TLS and decoding are native (_brevi_net, which reaches the network
# only in a program the compiler gave it to). The same steps run blocking (requests, httpx.Client) or on
# Brevi's asyncio (httpx.AsyncClient), waiting for the socket where CPython's selector would.
import _brevi_net


class Closed(Exception):
    pass


class Protocol(Exception):
    pass


class DecodeFailed(Exception):
    pass


class Timeout(Exception):
    # which wait ran out: "connect", "read", "write"
    def __init__(self, phase, msg):
        Exception.__init__(self, msg)
        self.phase = phase


def _latin1(b):
    return b.decode("latin-1")


# ---------- connections ----------

class Conn:
    def __init__(self, cid, key):
        self.id = cid
        self.key = key
        self.reusable = False
        self.used = 0

    def close(self):
        if self.id is not None:
            _brevi_net.close(self.id)
            self.id = None


def host_header(host, port, scheme):
    h = "[" + host + "]" if ":" in host else host
    if port is None or (scheme == "http" and port == 80) or (scheme == "https" and port == 443):
        return h
    return h + ":" + str(port)


def _tunnel_request(host, port, proxy_auth):
    target = ("[" + host + "]" if ":" in host else host) + ":" + str(port)
    lines = "CONNECT " + target + " HTTP/1.1\r\nHost: " + target + "\r\n"
    if proxy_auth:
        lines += "Proxy-Authorization: " + proxy_auth + "\r\n"
    return (lines + "\r\n").encode("latin-1")


def connect(scheme, host, port, timeout, tls, proxy=None):
    # tls: (verify, cafile, capath, check_hostname); proxy: (host, port, auth header or None)
    t = -1.0 if timeout is None else float(timeout)
    if proxy is not None:
        cid = _brevi_net.open(proxy[0], proxy[1], t)
        if scheme == "https":
            _brevi_net.sendall(cid, _tunnel_request(host, port, proxy[2]))
            status, reason, _ = _head_sync(cid)
            if status != 200:
                _brevi_net.close(cid)
                raise OSError("Tunnel connection failed: " + str(status) + " " + reason)
    else:
        cid = _brevi_net.open(host, port, t)
    if scheme == "https":
        try:
            _brevi_net.tls(cid, host, tls[0], tls[1] or "", tls[2] or "", tls[3])
            _brevi_net.handshake(cid)
        except BaseException:
            _brevi_net.close(cid)
            raise
    return cid


def _head_sync(cid):
    while True:
        h = _brevi_net.head(cid)
        if h is not None:
            return parse_head(h)
        if _brevi_net.fill(cid) == 0:
            raise Closed("Remote end closed connection without response")


def parse_head(h):
    lines = h.split(b"\n")
    first = lines[0].rstrip(b"\r")
    parts = first.split(b" ", 2)
    if len(parts) < 2 or not parts[0].startswith(b"HTTP/"):
        raise Protocol(first)
    try:
        status = int(parts[1].decode("latin-1"))
    except ValueError:
        raise Protocol(first)
    reason = _latin1(parts[2]) if len(parts) > 2 else ""
    headers = []
    for l in lines[1:]:
        l = l.rstrip(b"\r")
        if not l:
            continue
        if l[:1] in (b" ", b"\t") and headers:
            # an obsolete folded line continues the header before
            k, v = headers[-1]
            headers[-1] = (k, v + b" " + l.strip())
            continue
        i = l.find(b":")
        if i <= 0:
            continue
        headers.append((l[:i].strip(), l[i + 1:].strip()))
    return status, reason, (_latin1(parts[0]), headers)


def request_bytes(method, target, headers):
    out = method + " " + target + " HTTP/1.1\r\n"
    for k, v in headers:
        out += k + ": " + v + "\r\n"
    return (out + "\r\n").encode("latin-1") if out.isascii() else _mixed(out)


def _mixed(s):
    # header text outside latin-1 is sent as UTF-8, as http.client does with str values it cannot encode
    try:
        return (s + "\r\n").encode("latin-1")
    except UnicodeEncodeError:
        return (s + "\r\n").encode("utf-8")


def send_sync(cid, data):
    if data:
        _brevi_net.sendall(cid, data)


# ---------- reading a response, blocking ----------

def read_head(cid):
    # skips 1xx responses (100 Continue), as http.client and h11 do
    while True:
        status, reason, rest = _head_sync(cid)
        if status >= 200 or status == 101:
            return status, reason, rest[0], rest[1]


def framing(method, status, headers):
    # ("none",), ("length", n), ("chunked",), ("close",)
    if method == "HEAD" or status in (204, 304) or 100 <= status < 200:
        return ("none",)
    te = None
    cl = None
    for k, v in headers:
        lk = k.lower()
        if lk == b"transfer-encoding":
            te = v.lower()
        elif lk == b"content-length" and cl is None:
            try:
                cl = int(v.split(b",")[0].strip().decode("latin-1"))
            except ValueError:
                raise Protocol(b"Invalid Content-Length")
    if te is not None and b"chunked" in te:
        return ("chunked",)
    if cl is not None:
        return ("length", cl)
    return ("close",)


class Body:
    # The body of one response on one connection: next_sync() / next_async() give the bytes as they arrive,
    # b"" at the end.
    def __init__(self, cid, frame):
        self.cid = cid
        self.frame = frame[0]
        self.left = frame[1] if frame[0] == "length" else 0
        self.done = frame[0] == "none" or (frame[0] == "length" and self.left == 0)
        self.chunk_left = 0
        self.trailer = False
        self.clean = self.done

    def _step(self, want=0):
        # what can be taken from the buffer now: bytes, b"" at the end, or None (more must arrive). With want,
        # a chunked body gives min(want, what is left of the HTTP chunk) bytes at once, as urllib3 reads it.
        cid = self.cid
        if self.frame == "length":
            k = _brevi_net.avail(cid)
            if k == 0:
                return None
            b = _brevi_net.take(cid, min(k, self.left))
            self.left -= len(b)
            if self.left == 0:
                self.done = True
                self.clean = True
            return b
        if self.frame == "close":
            k = _brevi_net.avail(cid)
            if k == 0:
                return None
            return _brevi_net.take(cid, k)
        # chunked
        while True:
            if self.trailer:
                l = _brevi_net.line(cid)
                if l is None:
                    return None
                if l.strip() == b"":
                    self.done = True
                    self.clean = True
                    return b""
                continue
            if self.chunk_left > 0:
                k = _brevi_net.avail(cid)
                if k == 0 or (want and k < min(want, self.chunk_left)):
                    return None
                b = _brevi_net.take(cid, min(k, self.chunk_left, want) if want else min(k, self.chunk_left))
                self.chunk_left -= len(b)
                if self.chunk_left == 0:
                    self.chunk_left = -1
                return b
            if self.chunk_left == -1:
                l = _brevi_net.line(cid)
                if l is None:
                    return None
                self.chunk_left = 0
                continue
            l = _brevi_net.line(cid)
            if l is None:
                return None
            size = l.split(b";")[0].strip()
            try:
                n = int(size.decode("latin-1"), 16)
            except ValueError:
                raise Protocol(b"Invalid chunk size: " + l.strip())
            if n == 0:
                self.trailer = True
            else:
                self.chunk_left = n

    def _eof(self):
        if self.frame == "close":
            self.done = True
            return b""
        raise Closed("connection closed before the response ended")

    def next_sync(self, want=0):
        while not self.done:
            b = self._step(want)
            if b is None:
                if _brevi_net.fill(self.cid) == 0:
                    if self._step() is None:
                        return self._eof()
                continue
            if b or self.done:
                return b
        return b""

    async def next_async(self, wait, deadline):
        while not self.done:
            b = self._step()
            if b is None:
                got = _brevi_net.fill(self.cid)
                if got == 0:
                    if self._step() is None:
                        return self._eof()
                elif got < 0:
                    await wait(self.cid, 1 if got == -1 else 4, deadline)
                continue
            if b or self.done:
                return b
        return b""


# ---------- non-blocking steps, for httpx.AsyncClient ----------

async def connect_async(scheme, host, port, tls, wait, deadline, proxy=None):
    if proxy is not None:
        cid = _brevi_net.open_nb(proxy[0], proxy[1])
    else:
        cid = _brevi_net.open_nb(host, port)
    try:
        while not _brevi_net.ready(cid):
            await wait(cid, 4, deadline)
        if proxy is not None and scheme == "https":
            await send_async(cid, _tunnel_request(host, port, proxy[2]), wait, deadline)
            status, reason, _ = await _head_async(cid, wait, deadline)
            if status != 200:
                raise OSError("Tunnel connection failed: " + str(status) + " " + reason)
        if scheme == "https":
            _brevi_net.tls(cid, host, tls[0], tls[1] or "", tls[2] or "", tls[3])
            while True:
                w = _brevi_net.handshake(cid)
                if w == 0:
                    break
                await wait(cid, 1 if w == 1 else 4, deadline)
    except BaseException:
        _brevi_net.close(cid)
        raise
    return cid


async def send_async(cid, data, wait, deadline):
    at = 0
    while at < len(data):
        k = _brevi_net.send(cid, data[at:])
        if k < 0:
            await wait(cid, 1 if k == -1 else 4, deadline)
            continue
        at += k


async def _head_async(cid, wait, deadline):
    while True:
        h = _brevi_net.head(cid)
        if h is not None:
            return parse_head(h)
        got = _brevi_net.fill(cid)
        if got == 0:
            raise Closed("Remote end closed connection without response")
        if got < 0:
            await wait(cid, 1 if got == -1 else 4, deadline)


async def read_head_async(cid, wait, deadline):
    while True:
        status, reason, rest = await _head_async(cid, wait, deadline)
        if status >= 200 or status == 101:
            return status, reason, rest[0], rest[1]


# ---------- content decoding ----------

class Decoder:
    # Content-Encoding gzip / x-gzip / deflate, as urllib3 and httpx decode them: deflate is zlib data or,
    # when that fails at once, raw deflate; gzip may hold several members (urllib3 reads them all, httpx the first).
    def __init__(self, kind, multi=True):
        self.kind = kind
        self.multi = multi
        self.ended = False
        self.first = True
        self.z = _brevi_net.zopen("gzip" if kind == "gzip" else "zlib")
        self.raw = False
        self.seen = b""

    def feed(self, data):
        try:
            return self._feed(data)
        except DecodeFailed:
            raise
        except Exception as e:
            raise DecodeFailed(str(e))

    def _feed(self, data):
        if not data or self.ended:
            return b""
        if self.kind == "deflate" and self.first:
            self.seen += data
            try:
                out = _brevi_net.zfeed(self.z, data)
            except Exception:
                _brevi_net.zclose(self.z)
                self.z = _brevi_net.zopen("deflate")
                self.raw = True
                out = _brevi_net.zfeed(self.z, self.seen)
            if out or len(self.seen) > 2:
                self.first = False
                self.seen = b""
            return out
        out = _brevi_net.zfeed(self.z, data)
        if self.kind == "gzip" and not self.multi and _brevi_net.zstate(self.z) == 2:
            # httpx's decoder stops after the first gzip member
            self.ended = True
            return out
        while self.kind == "gzip" and _brevi_net.zstate(self.z) == 2:
            rest = _brevi_net.zrest(self.z)
            _brevi_net.zclose(self.z)
            self.z = _brevi_net.zopen("gzip")
            out += _brevi_net.zfeed(self.z, rest)
        return out

    def close(self):
        if self.z is not None:
            _brevi_net.zclose(self.z)
            self.z = None


def decoders(encoding_header):
    # the decoders for a Content-Encoding value ("gzip", "deflate, gzip"), last applied first; None when one
    # is not known (the body is then left as it came)
    out = []
    for e in encoding_header.split(","):
        e = e.strip().lower()
        if e in ("gzip", "x-gzip"):
            out.append(Decoder("gzip"))
        elif e == "deflate":
            out.append(Decoder("deflate"))
        elif e in ("", "identity"):
            continue
        else:
            return None
    out.reverse()
    return out


# ---------- proxies from the environment ----------

def env(name):
    v = _brevi_net.env(name)
    if v is None:
        v = _brevi_net.env(name.upper())
    return v


def env_exact(name):
    return _brevi_net.env(name)
