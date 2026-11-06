# Brevi's httpx: the API of httpx 0.28 (Client, AsyncClient, the module functions, streaming, URL,
# QueryParams, Headers, Cookies, Request, Response, Timeout, the exception classes, codes, BasicAuth and Auth
# flows) written over Brevi's own HTTP/1.1 (_brevi_http: native sockets and TLS). It follows httpx's code
# for what a program sees: the headers sent and their order, URL parsing and quoting, redirects, cookies,
# encodings and the error messages (those of httpcore and h11 for the sync client, anyio's for the async one).
# AsyncClient runs on Brevi's asyncio, waiting for its sockets where CPython's selector would. caps.rs lists
# what it has (OWN_HTTPX) and what sends a program to CPython's httpx instead (HTTPX_LACKS: http2, transports,
# mounts, client certificates, files=, ...). With verify=True it trusts the system's certificates (or
# $SSL_CERT_FILE / $SSL_CERT_DIR) where httpx trusts certifi's bundle. Ported from httpx (BSD-3-Clause, Encode OSS Ltd:
# src/dynpy/py/LICENSE-httpx.txt).
import _brevi_http
import _brevi_net
import asyncio
import base64
import datetime
import json as jsonlib
import time
from enum import IntEnum

__version__ = "0.28.1"
__title__ = "httpx"

# ---------- exceptions ----------


class HTTPError(Exception):
    def __init__(self, message):
        Exception.__init__(self, message)
        self._request = None

    @property
    def request(self):
        if self._request is None:
            raise RuntimeError("The .request property has not been set.")
        return self._request

    @request.setter
    def request(self, request):
        self._request = request


class RequestError(HTTPError):
    def __init__(self, message, *, request=None):
        HTTPError.__init__(self, message)
        self._request = request


class TransportError(RequestError):
    pass


class TimeoutException(TransportError):
    pass


class ConnectTimeout(TimeoutException):
    pass


class ReadTimeout(TimeoutException):
    pass


class WriteTimeout(TimeoutException):
    pass


class PoolTimeout(TimeoutException):
    pass


class NetworkError(TransportError):
    pass


class ReadError(NetworkError):
    pass


class WriteError(NetworkError):
    pass


class ConnectError(NetworkError):
    pass


class CloseError(NetworkError):
    pass


class ProxyError(TransportError):
    pass


class UnsupportedProtocol(TransportError):
    pass


class ProtocolError(TransportError):
    pass


class LocalProtocolError(ProtocolError):
    pass


class RemoteProtocolError(ProtocolError):
    pass


class DecodingError(RequestError):
    pass


class TooManyRedirects(RequestError):
    pass


class HTTPStatusError(HTTPError):
    def __init__(self, message, *, request, response):
        HTTPError.__init__(self, message)
        self._request = request
        self.response = response


class InvalidURL(Exception):
    pass


class CookieConflict(Exception):
    pass


class StreamError(RuntimeError):
    pass


class StreamConsumed(StreamError):
    def __init__(self):
        super().__init__("Attempted to read or stream some content, but the content has already been streamed. For requests, this could be due to passing a generator as request content, and then receiving a redirect response or a secondary request as part of an authentication flow.For responses, this could be due to attempting to stream the response content more than once.")


class StreamClosed(StreamError):
    def __init__(self):
        super().__init__("Attempted to read or stream content, but the stream has been closed.")


class ResponseNotRead(StreamError):
    def __init__(self):
        super().__init__("Attempted to access streaming response content, without having called `read()`.")


class RequestNotRead(StreamError):
    def __init__(self):
        super().__init__("Attempted to access streaming request content, without having called `read()`.")


# ---------- status codes ----------


class codes(IntEnum):
    CONTINUE = 100
    SWITCHING_PROTOCOLS = 101
    PROCESSING = 102
    EARLY_HINTS = 103
    OK = 200
    CREATED = 201
    ACCEPTED = 202
    NON_AUTHORITATIVE_INFORMATION = 203
    NO_CONTENT = 204
    RESET_CONTENT = 205
    PARTIAL_CONTENT = 206
    MULTI_STATUS = 207
    ALREADY_REPORTED = 208
    IM_USED = 226
    MULTIPLE_CHOICES = 300
    MOVED_PERMANENTLY = 301
    FOUND = 302
    SEE_OTHER = 303
    NOT_MODIFIED = 304
    USE_PROXY = 305
    TEMPORARY_REDIRECT = 307
    PERMANENT_REDIRECT = 308
    BAD_REQUEST = 400
    UNAUTHORIZED = 401
    PAYMENT_REQUIRED = 402
    FORBIDDEN = 403
    NOT_FOUND = 404
    METHOD_NOT_ALLOWED = 405
    NOT_ACCEPTABLE = 406
    PROXY_AUTHENTICATION_REQUIRED = 407
    REQUEST_TIMEOUT = 408
    CONFLICT = 409
    GONE = 410
    LENGTH_REQUIRED = 411
    PRECONDITION_FAILED = 412
    REQUEST_ENTITY_TOO_LARGE = 413
    REQUEST_URI_TOO_LONG = 414
    UNSUPPORTED_MEDIA_TYPE = 415
    REQUESTED_RANGE_NOT_SATISFIABLE = 416
    EXPECTATION_FAILED = 417
    IM_A_TEAPOT = 418
    MISDIRECTED_REQUEST = 421
    UNPROCESSABLE_ENTITY = 422
    LOCKED = 423
    FAILED_DEPENDENCY = 424
    TOO_EARLY = 425
    UPGRADE_REQUIRED = 426
    PRECONDITION_REQUIRED = 428
    TOO_MANY_REQUESTS = 429
    REQUEST_HEADER_FIELDS_TOO_LARGE = 431
    UNAVAILABLE_FOR_LEGAL_REASONS = 451
    INTERNAL_SERVER_ERROR = 500
    NOT_IMPLEMENTED = 501
    BAD_GATEWAY = 502
    SERVICE_UNAVAILABLE = 503
    GATEWAY_TIMEOUT = 504
    HTTP_VERSION_NOT_SUPPORTED = 505
    VARIANT_ALSO_NEGOTIATES = 506
    INSUFFICIENT_STORAGE = 507
    LOOP_DETECTED = 508
    NOT_EXTENDED = 510
    NETWORK_AUTHENTICATION_REQUIRED = 511

    def __str__(self):
        return str(self.value)

    @classmethod
    def get_reason_phrase(cls, value):
        return _PHRASES.get(value, "")

    @classmethod
    def is_informational(cls, value):
        return 100 <= value <= 199

    @classmethod
    def is_success(cls, value):
        return 200 <= value <= 299

    @classmethod
    def is_redirect(cls, value):
        return 300 <= value <= 399

    @classmethod
    def is_client_error(cls, value):
        return 400 <= value <= 499

    @classmethod
    def is_server_error(cls, value):
        return 500 <= value <= 599

    @classmethod
    def is_error(cls, value):
        return 400 <= value <= 599


_PHRASES = {
    100: "Continue", 101: "Switching Protocols", 102: "Processing", 103: "Early Hints", 200: "OK", 201: "Created",
    202: "Accepted", 203: "Non-Authoritative Information", 204: "No Content", 205: "Reset Content",
    206: "Partial Content", 207: "Multi-Status", 208: "Already Reported", 226: "IM Used", 300: "Multiple Choices",
    301: "Moved Permanently", 302: "Found", 303: "See Other", 304: "Not Modified", 305: "Use Proxy",
    307: "Temporary Redirect", 308: "Permanent Redirect", 400: "Bad Request", 401: "Unauthorized",
    402: "Payment Required", 403: "Forbidden", 404: "Not Found", 405: "Method Not Allowed", 406: "Not Acceptable",
    407: "Proxy Authentication Required", 408: "Request Timeout", 409: "Conflict", 410: "Gone",
    411: "Length Required", 412: "Precondition Failed", 413: "Request Entity Too Large", 414: "Request-URI Too Long",
    415: "Unsupported Media Type", 416: "Requested Range Not Satisfiable", 417: "Expectation Failed",
    418: "I'm a teapot", 421: "Misdirected Request", 422: "Unprocessable Entity", 423: "Locked",
    424: "Failed Dependency", 425: "Too Early", 426: "Upgrade Required", 428: "Precondition Required",
    429: "Too Many Requests", 431: "Request Header Fields Too Large", 451: "Unavailable For Legal Reasons",
    500: "Internal Server Error", 501: "Not Implemented", 502: "Bad Gateway", 503: "Service Unavailable",
    504: "Gateway Timeout", 505: "HTTP Version Not Supported", 506: "Variant Also Negotiates",
    507: "Insufficient Storage", 508: "Loop Detected", 510: "Not Extended", 511: "Network Authentication Required",
}
for _c in codes:
    _c.phrase = _PHRASES[_c.value]

# ---------- URL parsing (httpx._urlparse) ----------

_UNRESERVED = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~"
_SUB_DELIMS = "!$&'()*+,;="


def _printable_except(drop):
    return "".join(chr(i) for i in range(0x20, 0x7F) if i not in drop)


_FRAG_SAFE = _printable_except((0x20, 0x22, 0x3C, 0x3E, 0x60))
_QUERY_SAFE = _printable_except((0x20, 0x22, 0x23, 0x3C, 0x3E))
_PATH_SAFE = _printable_except((0x20, 0x22, 0x23, 0x3C, 0x3E, 0x3F, 0x60, 0x7B, 0x7D))
_USERNAME_SAFE = _printable_except((0x20, 0x22, 0x23, 0x3C, 0x3E, 0x3F, 0x60, 0x7B, 0x7D, 0x2F, 0x3A, 0x3B, 0x3D, 0x40, 0x5B, 0x5C, 0x5D, 0x5E, 0x7C))
_PASSWORD_SAFE = _USERNAME_SAFE
_USERINFO_SAFE = _printable_except((0x20, 0x22, 0x23, 0x3C, 0x3E, 0x3F, 0x60, 0x7B, 0x7D, 0x2F, 0x3B, 0x3D, 0x40, 0x5B, 0x5C, 0x5D, 0x5E, 0x7C))
_HEX = "0123456789abcdefABCDEF"
_MAX_URL_LENGTH = 65536
_DEFAULT_PORTS = {"ftp": 21, "http": 80, "https": 443, "ws": 80, "wss": 443}


def _percent_encoded(s, safe):
    ok = _UNRESERVED + safe
    if not s.rstrip(ok):
        return s
    out = []
    for ch in s:
        if ch in ok:
            out.append(ch)
        else:
            for b in ch.encode("utf-8"):
                out.append("%" + "%02X" % b)
    return "".join(out)


def _quote(s, safe):
    # percent-encode, keeping the %xx escapes already there
    parts = []
    start = 0
    i = 0
    n = len(s)
    while i < n:
        if s[i] == "%" and i + 2 < n and s[i + 1] in _HEX and s[i + 2] in _HEX:
            if i > start:
                parts.append(_percent_encoded(s[start:i], safe))
            parts.append(s[i:i + 3])
            i += 3
            start = i
            continue
        i += 1
    if start < n:
        parts.append(_percent_encoded(s[start:], safe))
    return "".join(parts)


def _is_scheme(s):
    if not s or not s[0].isascii() or not s[0].isalpha():
        return False
    for c in s:
        if not (c.isascii() and (c.isalnum() or c in "+-.")):
            return False
    return True


def _split_url(url):
    # URL_REGEX: scheme, authority, path, query, fragment (None where the part is absent)
    scheme = None
    rest = url
    i = url.find(":")
    if i >= 0:
        head = url[:i]
        if (head == "" or _is_scheme(head)) and not any(c in head for c in "/?#"):
            scheme = head
            rest = url[i + 1:]
    authority = None
    if rest.startswith("//"):
        end = len(rest)
        for c in "/?#":
            k = rest.find(c, 2)
            if k >= 0 and k < end:
                end = k
        authority = rest[2:end]
        rest = rest[end:]
    frag = None
    k = rest.find("#")
    if k >= 0:
        frag = rest[k + 1:]
        rest = rest[:k]
    query = None
    k = rest.find("?")
    if k >= 0:
        query = rest[k + 1:]
        rest = rest[:k]
    return scheme, authority, rest, query, frag


def _split_authority(a):
    userinfo = None
    k = a.rfind("@")
    if k >= 0:
        userinfo = a[:k]
        a = a[k + 1:]
    if a.startswith("[") and "]" in a:
        e = a.rfind("]")
        host = a[:e + 1]
        rest = a[e + 1:]
    else:
        e = len(a)
        for c in ":@":
            j = a.find(c)
            if j >= 0 and j < e:
                e = j
        host = a[:e]
        rest = a[e:]
    if rest.startswith(":"):
        rest = rest[1:]
    port = rest if rest != "" else None
    return userinfo, host, port


def _check_component(key, value):
    if key == "path" and ("?" in value or "#" in value):
        raise InvalidURL("Invalid URL component '" + key + "'")
    if key == "query" and "#" in value:
        raise InvalidURL("Invalid URL component '" + key + "'")
    if key in ("authority",) and any(c in value for c in "/?#"):
        raise InvalidURL("Invalid URL component '" + key + "'")
    if key == "userinfo" and "@" in value:
        raise InvalidURL("Invalid URL component '" + key + "'")
    if key == "scheme" and value and not _is_scheme(value):
        raise InvalidURL("Invalid URL component '" + key + "'")
    if key == "host" and not (value.startswith("[") and value.endswith("]")) and ":" in value:
        raise InvalidURL("Invalid URL component '" + key + "'")


def _nonprintable(s):
    for i, ch in enumerate(s):
        if ch.isascii() and not ch.isprintable():
            return i, ch
    return None


def _is_ipv4(h):
    parts = h.split(".")
    if len(parts) != 4:
        return False
    for p in parts:
        if not p.isdigit() or not p.isascii():
            return False
    return True


def _valid_ipv4(h):
    for p in h.split("."):
        if int(p) > 255 or (len(p) > 1 and p[0] == "0"):
            return False
    return True


def _valid_ipv6(h):
    if not h or any(c not in "0123456789abcdefABCDEF:." for c in h):
        return False
    if h.count("::") > 1:
        return False
    groups = h.split(":")
    if "." in groups[-1]:
        if not (_is_ipv4(groups[-1]) and _valid_ipv4(groups[-1])):
            return False
        groups = groups[:-1] + ["0", "0"]
    if "::" in h:
        return len([g for g in groups if g]) <= 7 and all(len(g) <= 4 for g in groups)
    return len(groups) == 8 and all(1 <= len(g) <= 4 for g in groups)


def _encode_host(host):
    if not host:
        return ""
    if _is_ipv4(host):
        if not _valid_ipv4(host):
            raise InvalidURL("Invalid IPv4 address: " + repr(host))
        return host
    if host.startswith("[") and host.endswith("]"):
        if not _valid_ipv6(host[1:-1]):
            raise InvalidURL("Invalid IPv6 address: " + repr(host))
        return host[1:-1]
    if host.isascii():
        return _quote(host.lower(), _SUB_DELIMS + '"`{}%|\\')
    try:
        return _idna_encode(host.lower())
    except ValueError:
        raise InvalidURL("Invalid IDNA hostname: " + repr(host))


def _normalize_port(port, scheme):
    if port is None or port == "":
        return None
    try:
        n = int(port)
    except ValueError:
        raise InvalidURL("Invalid port: " + repr(port))
    if n == _DEFAULT_PORTS.get(scheme):
        return None
    return n


def _normalize_path(path):
    if "." not in path:
        return path
    comps = path.split("/")
    if "." not in comps and ".." not in comps:
        return path
    out = []
    for c in comps:
        if c == ".":
            pass
        elif c == "..":
            if out and out != [""]:
                out.pop()
        else:
            out.append(c)
    return "/".join(out)


class _Parsed:
    def __init__(self, scheme, userinfo, host, port, path, query, fragment):
        self.scheme = scheme
        self.userinfo = userinfo
        self.host = host
        self.port = port
        self.path = path
        self.query = query
        self.fragment = fragment

    @property
    def authority(self):
        return (self.userinfo + "@" if self.userinfo else "") + ("[" + self.host + "]" if ":" in self.host else self.host) + (":" + str(self.port) if self.port is not None else "")

    @property
    def netloc(self):
        return ("[" + self.host + "]" if ":" in self.host else self.host) + (":" + str(self.port) if self.port is not None else "")

    def copy_with(self, kw):
        if not kw:
            return self
        d = {"scheme": self.scheme, "authority": self.authority, "path": self.path, "query": self.query, "fragment": self.fragment}
        for k in kw:
            d[k] = kw[k]
        return _urlparse("", d)

    def __str__(self):
        a = self.authority
        return (self.scheme + ":" if self.scheme else "") + ("//" + a if a else "") + self.path + ("?" + self.query if self.query is not None else "") + ("#" + self.fragment if self.fragment is not None else "")


def _urlparse(url, kw):
    if len(url) > _MAX_URL_LENGTH:
        raise InvalidURL("URL too long")
    bad = _nonprintable(url)
    if bad is not None:
        raise InvalidURL("Invalid non-printable ASCII character in URL, " + repr(bad[1]) + " at position " + str(bad[0]) + ".")
    kw = dict(kw)
    if "port" in kw:
        p = kw["port"]
        kw["port"] = str(p) if isinstance(p, int) else p
    if "netloc" in kw:
        netloc = kw.pop("netloc") or ""
        h, _, p = netloc.partition(":")
        kw["host"] = h
        kw["port"] = p
    if "username" in kw or "password" in kw:
        u = _quote(kw.pop("username", "") or "", _USERNAME_SAFE)
        pw = _quote(kw.pop("password", "") or "", _PASSWORD_SAFE)
        kw["userinfo"] = u + ":" + pw if pw else u
    if "raw_path" in kw:
        rp = kw.pop("raw_path") or ""
        p, sep, q = rp.partition("?")
        kw["path"] = p
        kw["query"] = q if sep else None
    if "host" in kw:
        h = kw.get("host") or ""
        if ":" in h and not (h.startswith("[") and h.endswith("]")):
            kw["host"] = "[" + h + "]"
    for key in kw:
        value = kw[key]
        if value is not None:
            if len(value) > _MAX_URL_LENGTH:
                raise InvalidURL("URL component '" + key + "' too long")
            bad = _nonprintable(value)
            if bad is not None:
                raise InvalidURL("Invalid non-printable ASCII character in URL " + key + " component, " + repr(bad[1]) + " at position " + str(bad[0]) + ".")
            _check_component(key, value)
    s, a, p, q, f = _split_url(url)
    scheme = (kw["scheme"] if "scheme" in kw else s) or ""
    authority = (kw["authority"] if "authority" in kw else a) or ""
    path = (kw["path"] if "path" in kw else p) or ""
    query = kw["query"] if "query" in kw else q
    frag = kw["fragment"] if "fragment" in kw else f
    ui, h, pt = _split_authority(authority)
    userinfo = (kw["userinfo"] if "userinfo" in kw else ui) or ""
    host = (kw["host"] if "host" in kw else h) or ""
    port = kw["port"] if "port" in kw else pt
    pscheme = scheme.lower()
    puserinfo = _quote(userinfo, _USERINFO_SAFE)
    phost = _encode_host(host)
    pport = _normalize_port(port, scheme)
    has_scheme = pscheme != ""
    has_authority = puserinfo != "" or phost != "" or pport is not None
    if has_authority and path and not path.startswith("/"):
        raise InvalidURL("For absolute URLs, path must be empty or begin with '/'")
    if not has_scheme and not has_authority:
        if path.startswith("//"):
            raise InvalidURL("Relative URLs cannot have a path starting with '//'")
        if path.startswith(":"):
            raise InvalidURL("Relative URLs cannot have a path starting with ':'")
    if has_scheme or has_authority:
        path = _normalize_path(path)
    return _Parsed(pscheme, puserinfo, phost, pport, _quote(path, _PATH_SAFE), None if query is None else _quote(query, _QUERY_SAFE), None if frag is None else _quote(frag, _FRAG_SAFE))


# ---------- IDNA (punycode, RFC 3492) for host names outside ASCII ----------

def _puny_adapt(delta, numpoints, first):
    delta = delta // 700 if first else delta // 2
    delta += delta // numpoints
    k = 0
    while delta > 455:
        delta //= 35
        k += 36
    return k + (36 * delta) // (delta + 38)


def _puny_digit(d):
    return chr(d + 22 + 75 * (d < 26))


def _punycode(label):
    n = 128
    delta = 0
    bias = 72
    basic = [c for c in label if ord(c) < 128]
    out = "".join(basic)
    h = len(basic)
    b = h
    if b:
        out += "-"
    cps = [ord(c) for c in label]
    while h < len(cps):
        m = min(c for c in cps if c >= n)
        delta += (m - n) * (h + 1)
        n = m
        for c in cps:
            if c < n:
                delta += 1
            if c == n:
                q = delta
                k = 36
                while True:
                    t = 1 if k <= bias else (26 if k >= bias + 26 else k - bias)
                    if q < t:
                        break
                    out += _puny_digit(t + (q - t) % (36 - t))
                    q = (q - t) // (36 - t)
                    k += 36
                out += _puny_digit(q)
                bias = _puny_adapt(delta, h + 1, h == b)
                delta = 0
                h += 1
        delta += 1
        n += 1
    return out


def _idna_encode(host):
    out = []
    for label in host.split("."):
        if label.isascii():
            out.append(label)
        else:
            if any(not (c.isalnum() or c == "-") for c in label):
                raise ValueError(label)
            out.append("xn--" + _punycode(label))
    return ".".join(out)


def _unpunycode(s):
    n = 128
    i = 0
    bias = 72
    k = s.rfind("-")
    out = list(s[:k]) if k > 0 else []
    pos = k + 1 if k > 0 else 0
    while pos < len(s):
        oldi = i
        w = 1
        kk = 36
        while True:
            c = s[pos]
            pos += 1
            d = ord(c) - 48 + 26 if c.isdigit() else ord(c.lower()) - 97
            i += d * w
            t = 1 if kk <= bias else (26 if kk >= bias + 26 else kk - bias)
            if d < t:
                break
            w *= 36 - t
            kk += 36
        bias = _puny_adapt(i - oldi, len(out) + 1, oldi == 0)
        n += i // (len(out) + 1)
        i %= len(out) + 1
        out.insert(i, chr(n))
        i += 1
    return "".join(out)


def _idna_decode(host):
    out = []
    for label in host.split("."):
        if label.startswith("xn--"):
            try:
                out.append(_unpunycode(label[4:]))
            except Exception:
                out.append(label)
        else:
            out.append(label)
    return ".".join(out)


# ---------- urllib.parse pieces: quote_plus, unquote, urlencode, parse_qs, urljoin ----------

_ALWAYS_SAFE = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_.-~"


def _quote_plus(s):
    b = s.encode("utf-8") if isinstance(s, str) else s
    out = []
    for c in b:
        ch = chr(c)
        if c < 128 and ch in _ALWAYS_SAFE:
            out.append(ch)
        elif ch == " ":
            out.append("+")
        else:
            out.append("%" + "%02X" % c)
    return "".join(out)


def _unquote(s):
    if "%" not in s:
        return s
    parts = s.split("%")
    out = [parts[0].encode("utf-8")]
    for p in parts[1:]:
        h = p[:2]
        if len(h) == 2 and h[0] in _HEX and h[1] in _HEX:
            out.append(bytes([int(h, 16)]) + p[2:].encode("utf-8"))
        else:
            out.append(b"%" + p.encode("utf-8"))
    return b"".join(out).decode("utf-8", "replace")


def _urlencode(pairs):
    return "&".join(_quote_plus(str(k)) + "=" + _quote_plus(v if isinstance(v, (str, bytes)) else str(v)) for k, v in pairs)


def _parse_qs(qs):
    d = {}
    if not qs:
        return d
    for field in qs.split("&"):
        if not field:
            continue
        k, sep, v = field.partition("=")
        k = _unquote(k.replace("+", " "))
        v = _unquote(v.replace("+", " "))
        if k in d:
            d[k].append(v)
        else:
            d[k] = [v]
    return d


def _urlsplit(url):
    scheme = ""
    i = url.find(":")
    if i > 0 and url[0].isascii() and url[0].isalpha() and all(c.isascii() and (c.isalnum() or c in "+-.") for c in url[:i]):
        scheme = url[:i].lower()
        url = url[i + 1:]
    netloc = ""
    if url[:2] == "//":
        end = len(url)
        for c in "/?#":
            k = url.find(c, 2)
            if k >= 0:
                end = min(end, k)
        netloc = url[2:end]
        url = url[end:]
    fragment = ""
    if "#" in url:
        url, fragment = url.split("#", 1)
    query = ""
    if "?" in url:
        url, query = url.split("?", 1)
    return scheme, netloc, url, query, fragment


def _urlunsplit(scheme, netloc, url, query, fragment):
    if netloc or (scheme and scheme in ("http", "https", "ws", "wss", "ftp") and url[:2] != "//"):
        if url and url[:1] != "/":
            url = "/" + url
        url = "//" + (netloc or "") + url
    if scheme:
        url = scheme + ":" + url
    if query:
        url = url + "?" + query
    if fragment:
        url = url + "#" + fragment
    return url


def _urljoin(base, url):
    # urllib.parse.urljoin for http(s) URLs
    if not base:
        return url
    if not url:
        return base
    bscheme, bnetloc, bpath, bquery, bfragment = _urlsplit(base)
    scheme, netloc, path, query, fragment = _urlsplit(url)
    if not scheme:
        scheme = bscheme
    if scheme != bscheme:
        return url
    if netloc:
        return _urlunsplit(scheme, netloc, path, query, fragment)
    netloc = bnetloc
    if not path:
        path = bpath
        if not query:
            query = bquery
        return _urlunsplit(scheme, netloc, path, query, fragment)
    base_parts = bpath.split("/")
    if base_parts[-1] != "":
        del base_parts[-1]
    if path[:1] == "/":
        segments = path.split("/")
    else:
        segments = base_parts + path.split("/")
        segments[1:-1] = [s for s in segments[1:-1] if s]
    resolved = []
    for seg in segments:
        if seg == "..":
            if resolved:
                resolved.pop()
        elif seg == ".":
            continue
        else:
            resolved.append(seg)
    if segments[-1] in (".", ".."):
        resolved.append("")
    return _urlunsplit(scheme, netloc, "/".join(resolved) or "/", query, fragment)


def _primitive(v):
    if v is True:
        return "true"
    if v is False:
        return "false"
    if v is None:
        return ""
    return str(v)


# ---------- URL and QueryParams ----------

_URL_KW = {"scheme": str, "username": str, "password": str, "userinfo": bytes, "host": str, "port": int, "netloc": bytes, "path": str, "query": bytes, "raw_path": bytes, "fragment": str, "params": object}


class URL:
    def __init__(self, url="", **kwargs):
        if kwargs:
            for key in kwargs:
                value = kwargs[key]
                if key not in _URL_KW:
                    raise TypeError(repr(key) + " is an invalid keyword argument for URL()")
                want = _URL_KW[key]
                if value is not None and want is not object and not isinstance(value, want):
                    raise TypeError("Argument " + repr(key) + " must be " + want.__name__ + " but got " + type(value).__name__)
                if isinstance(value, bytes):
                    kwargs[key] = value.decode("ascii")
            if "params" in kwargs:
                params = kwargs.pop("params")
                kwargs["query"] = None if not params else str(QueryParams(params))
        if isinstance(url, str):
            self._uri = _urlparse(url, kwargs)
        elif isinstance(url, URL):
            self._uri = url._uri.copy_with(kwargs)
        else:
            raise TypeError("Invalid type for url.  Expected str or httpx.URL, got " + str(type(url)) + ": " + repr(url))

    @property
    def scheme(self):
        return self._uri.scheme

    @property
    def raw_scheme(self):
        return self._uri.scheme.encode("ascii")

    @property
    def userinfo(self):
        return self._uri.userinfo.encode("ascii")

    @property
    def username(self):
        return _unquote(self._uri.userinfo.partition(":")[0])

    @property
    def password(self):
        return _unquote(self._uri.userinfo.partition(":")[2])

    @property
    def host(self):
        h = self._uri.host
        if h.startswith("xn--"):
            h = _idna_decode(h)
        return h

    @property
    def raw_host(self):
        return self._uri.host.encode("ascii")

    @property
    def port(self):
        return self._uri.port

    @property
    def netloc(self):
        return self._uri.netloc.encode("ascii")

    @property
    def path(self):
        return _unquote(self._uri.path or "/")

    @property
    def query(self):
        return (self._uri.query or "").encode("ascii")

    @property
    def params(self):
        return QueryParams(self._uri.query)

    @property
    def raw_path(self):
        p = self._uri.path or "/"
        if self._uri.query is not None:
            p += "?" + self._uri.query
        return p.encode("ascii")

    @property
    def fragment(self):
        return _unquote(self._uri.fragment or "")

    @property
    def is_absolute_url(self):
        return bool(self._uri.scheme and self._uri.host)

    @property
    def is_relative_url(self):
        return not self.is_absolute_url

    def copy_with(self, **kwargs):
        return URL(self, **kwargs)

    def copy_set_param(self, key, value=None):
        return self.copy_with(params=self.params.set(key, value))

    def copy_add_param(self, key, value=None):
        return self.copy_with(params=self.params.add(key, value))

    def copy_remove_param(self, key):
        return self.copy_with(params=self.params.remove(key))

    def copy_merge_params(self, params):
        return self.copy_with(params=self.params.merge(params))

    def join(self, url):
        return URL(_urljoin(str(self), str(URL(url))))

    def __hash__(self):
        return hash(str(self))

    def __eq__(self, other):
        return isinstance(other, (URL, str)) and str(self) == str(URL(other))

    def __str__(self):
        return str(self._uri)

    def __repr__(self):
        u = self._uri
        userinfo = u.userinfo
        if ":" in userinfo:
            userinfo = userinfo.split(":")[0] + ":[secure]"
        authority = (userinfo + "@" if userinfo else "") + ("[" + u.host + "]" if ":" in u.host else u.host) + (":" + str(u.port) if u.port is not None else "")
        url = (u.scheme + ":" if u.scheme else "") + ("//" + authority if authority else "") + u.path + ("?" + u.query if u.query is not None else "") + ("#" + u.fragment if u.fragment is not None else "")
        return "URL(" + repr(url) + ")"


class QueryParams:
    def __init__(self, *args, **kwargs):
        assert len(args) < 2, "Too many arguments."
        assert not (args and kwargs), "Cannot mix named and unnamed arguments."
        value = args[0] if args else kwargs
        if value is None or isinstance(value, (str, bytes)):
            value = value.decode("ascii") if isinstance(value, bytes) else value
            self._dict = _parse_qs(value)
        elif isinstance(value, QueryParams):
            self._dict = {k: list(v) for k, v in value._dict.items()}
        else:
            dv = {}
            if isinstance(value, (list, tuple)):
                for item in value:
                    dv.setdefault(item[0], []).append(item[1])
            else:
                for k, v in value.items():
                    dv[k] = list(v) if isinstance(v, (list, tuple)) else [v]
            self._dict = {str(k): [_primitive(x) for x in v] for k, v in dv.items()}

    def keys(self):
        return self._dict.keys()

    def values(self):
        return {k: v[0] for k, v in self._dict.items()}.values()

    def items(self):
        return {k: v[0] for k, v in self._dict.items()}.items()

    def multi_items(self):
        out = []
        for k, v in self._dict.items():
            out.extend([(k, i) for i in v])
        return out

    def get(self, key, default=None):
        if key in self._dict:
            return self._dict[str(key)][0]
        return default

    def get_list(self, key):
        return list(self._dict.get(str(key), []))

    def set(self, key, value=None):
        q = QueryParams()
        q._dict = dict(self._dict)
        q._dict[str(key)] = [_primitive(value)]
        return q

    def add(self, key, value=None):
        q = QueryParams()
        q._dict = dict(self._dict)
        q._dict[str(key)] = q.get_list(key) + [_primitive(value)]
        return q

    def remove(self, key):
        q = QueryParams()
        q._dict = dict(self._dict)
        q._dict.pop(str(key), None)
        return q

    def merge(self, params=None):
        q = QueryParams(params)
        d = dict(self._dict)
        for k, v in q._dict.items():
            d[k] = v
        q._dict = d
        return q

    def __getitem__(self, key):
        return self._dict[key][0]

    def __contains__(self, key):
        return key in self._dict

    def __iter__(self):
        return iter(self.keys())

    def __len__(self):
        return len(self._dict)

    def __bool__(self):
        return bool(self._dict)

    def __hash__(self):
        return hash(str(self))

    def __eq__(self, other):
        if not isinstance(other, QueryParams):
            return False
        return sorted(self.multi_items()) == sorted(other.multi_items())

    def __str__(self):
        return _urlencode(self.multi_items())

    def __repr__(self):
        return "QueryParams(" + repr(str(self)) + ")"

    def update(self, params=None):
        raise RuntimeError("QueryParams are immutable since 0.18.0. Use `q = q.merge(...)` to create an updated copy.")

    def __setitem__(self, key, value):
        raise RuntimeError("QueryParams are immutable since 0.18.0. Use `q = q.set(key, value)` to create an updated copy.")


# ---------- Headers ----------

def _hkey(k, encoding):
    return k if isinstance(k, bytes) else k.encode(encoding or "ascii")


def _hvalue(v, encoding):
    if isinstance(v, bytes):
        return v
    if not isinstance(v, str):
        raise TypeError("Header value must be str or bytes, not " + str(type(v)))
    return v.encode(encoding or "ascii")


_SENSITIVE = ("authorization", "proxy-authorization")


class Headers:
    def __init__(self, headers=None, encoding=None):
        self._list = []
        if isinstance(headers, Headers):
            self._list = list(headers._list)
        elif headers is not None and hasattr(headers, "items"):
            for k, v in headers.items():
                bk = _hkey(k, encoding)
                self._list.append((bk, bk.lower(), _hvalue(v, encoding)))
        elif headers is not None:
            for k, v in headers:
                bk = _hkey(k, encoding)
                self._list.append((bk, bk.lower(), _hvalue(v, encoding)))
        self._encoding = encoding

    @property
    def encoding(self):
        if self._encoding is None:
            for enc in ("ascii", "utf-8"):
                ok = True
                for k, _, v in self._list:
                    try:
                        k.decode(enc)
                        v.decode(enc)
                    except UnicodeDecodeError:
                        ok = False
                        break
                if ok:
                    self._encoding = enc
                    break
            else:
                self._encoding = "iso-8859-1"
        return self._encoding

    @encoding.setter
    def encoding(self, value):
        self._encoding = value

    @property
    def raw(self):
        return [(k, v) for k, _, v in self._list]

    def keys(self):
        enc = self.encoding
        return {k.decode(enc): None for _, k, v in self._list}.keys()

    def _joined(self):
        enc = self.encoding
        d = {}
        for _, k, v in self._list:
            sk = k.decode(enc)
            sv = v.decode(enc)
            if sk in d:
                d[sk] += ", " + sv
            else:
                d[sk] = sv
        return d

    def values(self):
        return self._joined().values()

    def items(self):
        return self._joined().items()

    def multi_items(self):
        enc = self.encoding
        return [(k.decode(enc), v.decode(enc)) for _, k, v in self._list]

    def get(self, key, default=None):
        try:
            return self[key]
        except KeyError:
            return default

    def get_list(self, key, split_commas=False):
        enc = self.encoding
        lk = key.lower().encode(enc)
        values = [v.decode(enc) for _, k, v in self._list if k.lower() == lk]
        if not split_commas:
            return values
        out = []
        for v in values:
            out.extend([x.strip() for x in v.split(",")])
        return out

    def update(self, headers=None):
        headers = Headers(headers)
        for key in headers.keys():
            if key in self:
                self.pop(key)
        self._list.extend(headers._list)

    def copy(self):
        return Headers(self, encoding=self.encoding)

    def pop(self, key, *default):
        try:
            v = self[key]
        except KeyError:
            if default:
                return default[0]
            raise
        del self[key]
        return v

    def setdefault(self, key, default=None):
        try:
            return self[key]
        except KeyError:
            self[key] = default
            return default

    def __getitem__(self, key):
        enc = self.encoding
        nk = key.lower().encode(enc)
        items = [v.decode(enc) for _, k, v in self._list if k == nk]
        if items:
            return ", ".join(items)
        raise KeyError(key)

    def __setitem__(self, key, value):
        sk = key.encode(self._encoding or "utf-8")
        sv = value.encode(self._encoding or "utf-8")
        lk = sk.lower()
        found = [i for i, (_, k, _) in enumerate(self._list) if k == lk]
        for i in reversed(found[1:]):
            del self._list[i]
        if found:
            self._list[found[0]] = (sk, lk, sv)
        else:
            self._list.append((sk, lk, sv))

    def __delitem__(self, key):
        dk = key.lower().encode(self.encoding)
        idx = [i for i, (_, k, _) in enumerate(self._list) if k.lower() == dk]
        if not idx:
            raise KeyError(key)
        for i in reversed(idx):
            del self._list[i]

    def __contains__(self, key):
        hk = key.lower().encode(self.encoding)
        return hk in [k for _, k, _ in self._list]

    def __iter__(self):
        return iter(self.keys())

    def __len__(self):
        return len(self._list)

    def __eq__(self, other):
        try:
            oh = Headers(other)
        except ValueError:
            return False
        return sorted([(k, v) for _, k, v in self._list]) == sorted([(k, v) for _, k, v in oh._list])

    def __repr__(self):
        enc = ""
        if self.encoding != "ascii":
            enc = ", encoding=" + repr(self.encoding)
        as_list = [(k, "[secure]" if k.lower() in _SENSITIVE else v) for k, v in self.multi_items()]
        as_dict = dict(as_list)
        if len(as_dict) == len(as_list):
            return "Headers(" + repr(as_dict) + enc + ")"
        return "Headers(" + repr(as_list) + enc + ")"


# ---------- cookies (http.cookiejar's rules for what a program can see) ----------

class Cookie:
    def __init__(self, name, value, domain, path, host_only, secure, domain_specified=False):
        self.name = name
        self.value = value
        self.domain = domain
        self.path = path
        self.host_only = host_only
        self.secure = secure
        self.domain_specified = domain_specified

    def __repr__(self):
        return "Cookie(version=0, name=" + repr(self.name) + ", value=" + repr(self.value) + ", port=None, port_specified=False, domain=" + repr(self.domain) + ", domain_specified=" + str(self.domain_specified) + ", domain_initial_dot=" + str(self.domain.startswith(".")) + ", path=" + repr(self.path) + ", path_specified=True, secure=" + str(self.secure) + ", expires=None, discard=True, comment=None, comment_url=None, rest={'HttpOnly': None}, rfc2109=False)"


class _Jar:
    def __init__(self):
        self._cookies = []

    def set_cookie(self, c):
        self._cookies = [x for x in self._cookies if not (x.name == c.name and x.domain == c.domain and x.path == c.path)]
        self._cookies.append(c)

    def clear(self, domain=None, path=None, name=None):
        self._cookies = [c for c in self._cookies if not ((domain is None or c.domain == domain) and (path is None or c.path == path) and (name is None or c.name == name))]

    def __iter__(self):
        return iter(list(self._cookies))

    def __len__(self):
        return len(self._cookies)

    def header_for(self, host, path, secure):
        out = []
        for c in sorted(self._cookies, key=lambda c: -len(c.path)):
            if c.domain:
                d = c.domain.lstrip(".")
                if c.host_only:
                    if host != d:
                        continue
                elif host != d and not host.endswith("." + d):
                    continue
            if not (path == c.path or path.startswith(c.path.rstrip("/") + "/") or c.path == "/"):
                continue
            if c.secure and not secure:
                continue
            out.append(c.name + "=" + c.value)
        return "; ".join(out) if out else None

    def extract(self, host, path, headers):
        for k, v in headers:
            if k.lower() != "set-cookie":
                continue
            parts = [p.strip() for p in v.split(";")]
            if "=" not in parts[0]:
                continue
            name, value = parts[0].split("=", 1)
            domain = None
            cpath = None
            secure = False
            expired = False
            for a in parts[1:]:
                ak, _, av = a.partition("=")
                ak = ak.strip().lower()
                if ak == "domain" and av:
                    domain = "." + av.strip().lstrip(".").lower()
                elif ak == "path" and av.startswith("/"):
                    cpath = av
                elif ak == "secure":
                    secure = True
                elif ak == "max-age" and av.strip().lstrip("-").isdigit() and int(av) <= 0:
                    expired = True
            if cpath is None:
                cpath = path[:path.rfind("/")] if path.count("/") > 1 else "/"
            host_only = domain is None
            c = Cookie(name.strip(), value.strip(), host if host_only else domain, cpath, host_only, secure, not host_only)
            if expired:
                self.clear(c.domain, c.path, c.name)
            else:
                self.set_cookie(c)


class Cookies:
    def __init__(self, cookies=None):
        if cookies is None or isinstance(cookies, dict):
            self.jar = _Jar()
            if isinstance(cookies, dict):
                for k, v in cookies.items():
                    self.set(k, v)
        elif isinstance(cookies, list):
            self.jar = _Jar()
            for k, v in cookies:
                self.set(k, v)
        elif isinstance(cookies, Cookies):
            self.jar = _Jar()
            for c in cookies.jar:
                self.jar.set_cookie(c)
        else:
            self.jar = cookies

    def extract_cookies(self, response):
        url = response.request.url
        self.jar.extract(url.host, url.path, response.headers.multi_items())

    def set_cookie_header(self, request):
        url = request.url
        h = self.jar.header_for(url.host, url.path, url.scheme == "https")
        if h is not None:
            request.headers["Cookie"] = h

    def set(self, name, value, domain="", path="/"):
        self.jar.set_cookie(Cookie(name, value, domain, path, False, False, bool(domain)))

    def get(self, name, default=None, domain=None, path=None):
        value = None
        for c in self.jar:
            if c.name == name and (domain is None or c.domain == domain) and (path is None or c.path == path):
                if value is not None:
                    raise CookieConflict("Multiple cookies exist with name=" + name)
                value = c.value
        if value is None:
            return default
        return value

    def delete(self, name, domain=None, path=None):
        if domain is not None and path is not None:
            return self.jar.clear(domain, path, name)
        for c in [c for c in self.jar if c.name == name and (domain is None or c.domain == domain) and (path is None or c.path == path)]:
            self.jar.clear(c.domain, c.path, c.name)

    def clear(self, domain=None, path=None):
        self.jar.clear(domain, path)

    def update(self, cookies=None):
        cookies = Cookies(cookies)
        for c in cookies.jar:
            self.jar.set_cookie(c)

    def keys(self):
        return [c.name for c in self.jar]

    def values(self):
        return [self[c.name] for c in self.jar]

    def items(self):
        return [(c.name, self[c.name]) for c in self.jar]

    def __setitem__(self, name, value):
        self.set(name, value)

    def __getitem__(self, name):
        v = self.get(name)
        if v is None:
            raise KeyError(name)
        return v

    def __delitem__(self, name):
        self.delete(name)

    def __contains__(self, name):
        return any(c.name == name for c in self.jar)

    def __len__(self):
        return len(self.jar)

    def __iter__(self):
        return iter([c.name for c in self.jar])

    def __bool__(self):
        return len(self.jar) > 0

    def __repr__(self):
        return "<Cookies[" + ", ".join("<Cookie " + c.name + "=" + c.value + " for " + c.domain + " />" for c in self.jar) + "]>"


# ---------- configuration ----------

class _Unset:
    pass


_UNSET = _Unset()


class Timeout:
    def __init__(self, timeout=_UNSET, *, connect=_UNSET, read=_UNSET, write=_UNSET, pool=_UNSET):
        if isinstance(timeout, Timeout):
            self.connect = timeout.connect
            self.read = timeout.read
            self.write = timeout.write
            self.pool = timeout.pool
        elif isinstance(timeout, tuple):
            self.connect = timeout[0]
            self.read = timeout[1]
            self.write = None if len(timeout) < 3 else timeout[2]
            self.pool = None if len(timeout) < 4 else timeout[3]
        elif not (connect is _UNSET or read is _UNSET or write is _UNSET or pool is _UNSET):
            self.connect = connect
            self.read = read
            self.write = write
            self.pool = pool
        else:
            if timeout is _UNSET:
                raise ValueError("httpx.Timeout must either include a default, or set all four parameters explicitly.")
            self.connect = timeout if connect is _UNSET else connect
            self.read = timeout if read is _UNSET else read
            self.write = timeout if write is _UNSET else write
            self.pool = timeout if pool is _UNSET else pool

    def as_dict(self):
        return {"connect": self.connect, "read": self.read, "write": self.write, "pool": self.pool}

    def __eq__(self, other):
        return isinstance(other, Timeout) and self.connect == other.connect and self.read == other.read and self.write == other.write and self.pool == other.pool

    def __repr__(self):
        if len({self.connect, self.read, self.write, self.pool}) == 1:
            return "Timeout(timeout=" + str(self.connect) + ")"
        return "Timeout(connect=" + str(self.connect) + ", read=" + str(self.read) + ", write=" + str(self.write) + ", pool=" + str(self.pool) + ")"


class Limits:
    def __init__(self, *, max_connections=None, max_keepalive_connections=None, keepalive_expiry=5.0):
        self.max_connections = max_connections
        self.max_keepalive_connections = max_keepalive_connections
        self.keepalive_expiry = keepalive_expiry

    def __eq__(self, other):
        return isinstance(other, Limits) and self.max_connections == other.max_connections and self.max_keepalive_connections == other.max_keepalive_connections and self.keepalive_expiry == other.keepalive_expiry

    def __repr__(self):
        return "Limits(max_connections=" + str(self.max_connections) + ", max_keepalive_connections=" + str(self.max_keepalive_connections) + ", keepalive_expiry=" + str(self.keepalive_expiry) + ")"


class Proxy:
    def __init__(self, url, *, ssl_context=None, auth=None, headers=None):
        url = URL(url)
        headers = Headers(headers)
        if url.scheme not in ("http", "https", "socks5", "socks5h"):
            raise ValueError("Unknown scheme for proxy URL " + repr(url))
        if url.username or url.password:
            auth = (url.username, url.password)
            url = url.copy_with(username=None, password=None)
        self.url = url
        self.auth = auth
        self.headers = headers
        self.ssl_context = ssl_context

    @property
    def raw_auth(self):
        return None if self.auth is None else (self.auth[0].encode("utf-8"), self.auth[1].encode("utf-8"))

    def __repr__(self):
        auth = (self.auth[0], "********") if self.auth else None
        return "Proxy(" + repr(str(self.url)) + (", auth=" + repr(auth) if auth else "") + (", headers=" + repr(dict(self.headers)) if self.headers else "") + ")"


DEFAULT_TIMEOUT_CONFIG = Timeout(timeout=5.0)
DEFAULT_LIMITS = Limits(max_connections=100, max_keepalive_connections=20)
DEFAULT_MAX_REDIRECTS = 20
USER_AGENT = "python-httpx/" + __version__
ACCEPT_ENCODING = "gzip, deflate"


class UseClientDefault:
    pass


USE_CLIENT_DEFAULT = UseClientDefault()


# ---------- auth ----------

class Auth:
    requires_request_body = False
    requires_response_body = False

    def auth_flow(self, request):
        yield request


class FunctionAuth(Auth):
    def __init__(self, func):
        self._func = func

    def auth_flow(self, request):
        yield self._func(request)


def _to_bytes(v):
    return v.encode("utf-8") if isinstance(v, str) else v


class BasicAuth(Auth):
    def __init__(self, username, password):
        self._auth_header = "Basic " + base64.b64encode(b":".join((_to_bytes(username), _to_bytes(password)))).decode()

    def auth_flow(self, request):
        request.headers["Authorization"] = self._auth_header
        yield request


# ---------- request bodies ----------

class ByteStream:
    def __init__(self, data):
        self._stream = data

    def __iter__(self):
        yield self._stream

    async def __aiter__(self):
        yield self._stream

    def close(self):
        pass

    async def aclose(self):
        pass


class _IterStream:
    # a body from an iterator (sent chunked); it can be sent once
    def __init__(self, it, is_async):
        self._it = it
        self._async = is_async
        self._used = False


def _encode_content(content):
    if isinstance(content, (bytes, str)):
        body = content.encode("utf-8") if isinstance(content, str) else content
        return {"Content-Length": str(len(body))} if body else {}, ByteStream(body)
    if hasattr(content, "__aiter__"):
        return {"Transfer-Encoding": "chunked"}, _IterStream(content, True)
    if hasattr(content, "__iter__"):
        return {"Transfer-Encoding": "chunked"}, _IterStream(content, False)
    raise TypeError("Unexpected type for 'content', " + repr(type(content)))


def _encode_request(content=None, data=None, files=None, json=None):
    if data is not None and not hasattr(data, "items"):
        return _encode_content(data)
    if content is not None:
        return _encode_content(content)
    if data:
        plain = []
        for k, v in data.items():
            if isinstance(v, (list, tuple)):
                plain.extend([(k, _primitive(x)) for x in v])
            else:
                plain.append((k, _primitive(v)))
        body = _urlencode(plain).encode("utf-8")
        return {"Content-Length": str(len(body)), "Content-Type": "application/x-www-form-urlencoded"}, ByteStream(body)
    if json is not None:
        body = jsonlib.dumps(json, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
        return {"Content-Length": str(len(body)), "Content-Type": "application/json"}, ByteStream(body)
    return {}, ByteStream(b"")


def _encode_response(content=None, text=None, html=None, json=None):
    if content is not None:
        return _encode_content(content)
    if text is not None:
        body = text.encode("utf-8")
        return {"Content-Length": str(len(body)), "Content-Type": "text/plain; charset=utf-8"}, ByteStream(body)
    if html is not None:
        body = html.encode("utf-8")
        return {"Content-Length": str(len(body)), "Content-Type": "text/html; charset=utf-8"}, ByteStream(body)
    if json is not None:
        body = jsonlib.dumps(json, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
        return {"Content-Length": str(len(body)), "Content-Type": "application/json"}, ByteStream(body)
    return {}, ByteStream(b"")


# ---------- Request ----------

class Request:
    def __init__(self, method, url, *, params=None, headers=None, cookies=None, content=None, data=None, files=None, json=None, stream=None, extensions=None):
        if files:
            raise NotImplementedError("Brevi's httpx does not send files=")
        self.method = method.upper() if isinstance(method, str) else method.decode("ascii").upper()
        self.url = URL(url) if params is None else URL(url, params=params)
        self.headers = Headers(headers)
        self.extensions = {} if extensions is None else dict(extensions)
        if cookies:
            Cookies(cookies).set_cookie_header(self)
        if stream is None:
            defaults, stream = _encode_request(content, data, files, json)
            self._prepare(defaults)
            self.stream = stream
            if isinstance(stream, ByteStream):
                self._content = stream._stream
        else:
            self.stream = stream
            if isinstance(stream, ByteStream):
                self._content = stream._stream

    def _prepare(self, defaults):
        for k, v in defaults.items():
            if k.lower() == "transfer-encoding" and "Content-Length" in self.headers:
                continue
            self.headers.setdefault(k, v)
        auto = []
        has_host = "Host" in self.headers
        has_cl = "Content-Length" in self.headers or "Transfer-Encoding" in self.headers
        if not has_host and self.url.host:
            auto.append((b"Host", self.url.netloc))
        if not has_cl and self.method in ("POST", "PUT", "PATCH"):
            auto.append((b"Content-Length", b"0"))
        self.headers = Headers(auto + self.headers.raw)

    @property
    def content(self):
        if not hasattr(self, "_content"):
            raise RequestNotRead()
        return self._content

    def read(self):
        if not hasattr(self, "_content"):
            s = self.stream
            if isinstance(s, _IterStream):
                if s._async:
                    raise RuntimeError("Attempted to read an async request body synchronously.")
                self._content = b"".join(_to_bytes(x) for x in s._it)
            else:
                self._content = b"".join(s)
            self.stream = ByteStream(self._content)
        return self._content

    async def aread(self):
        if not hasattr(self, "_content"):
            s = self.stream
            parts = []
            if isinstance(s, _IterStream) and s._async:
                async for x in s._it:
                    parts.append(_to_bytes(x))
            elif isinstance(s, _IterStream):
                for x in s._it:
                    parts.append(_to_bytes(x))
            else:
                for x in s:
                    parts.append(x)
            self._content = b"".join(parts)
            self.stream = ByteStream(self._content)
        return self._content

    def __repr__(self):
        return "<Request(" + repr(self.method) + ", " + repr(str(self.url)) + ")>"


# ---------- Response ----------

def _content_type_charset(ct):
    # email.message.Message.get_content_charset: the charset parameter, unquoted and lowercased
    for p in ct.split(";")[1:]:
        k, sep, v = p.partition("=")
        if sep and k.strip().lower() == "charset":
            v = v.strip()
            if len(v) >= 2 and v[0] == v[-1] and v[0] in "\"'":
                v = v[1:-1]
            try:
                v.encode("us-ascii")
            except UnicodeError:
                return None
            return v.lower()
    return None


def _known_encoding(enc):
    try:
        b"".decode(enc)
    except LookupError:
        return False
    return True


def _parse_header_links(value):
    links = []
    rc = " '\""
    value = value.strip(rc)
    if not value:
        return links
    vals = []
    cur = value
    while True:
        k = cur.find(",")
        found = -1
        while k >= 0:
            j = k + 1
            while j < len(cur) and cur[j] == " ":
                j += 1
            if j < len(cur) and cur[j] == "<":
                found = k
                break
            k = cur.find(",", k + 1)
        if found < 0:
            vals.append(cur)
            break
        vals.append(cur[:found])
        j = found + 1
        while cur[j] == " ":
            j += 1
        cur = cur[j + 1:]
    for val in vals:
        if ";" in val:
            url, params = val.split(";", 1)
        else:
            url, params = val, ""
        link = {"url": url.strip("<> '\"")}
        for param in params.split(";"):
            kv = param.split("=")
            if len(kv) != 2:
                break
            link[kv[0].strip(rc)] = kv[1].strip(rc)
        links.append(link)
    return links


def _json_encoding(data):
    # json.detect_encoding
    if data.startswith(b"\xef\xbb\xbf"):
        return "utf-8-sig"
    if data.startswith((b"\xff\xfe\x00\x00", b"\x00\x00\xfe\xff")):
        return "utf-32"
    if data.startswith((b"\xff\xfe", b"\xfe\xff")):
        return "utf-16"
    if len(data) >= 4:
        if not data[0]:
            return "utf-16-be" if data[1] else "utf-32-be"
        if not data[1]:
            return "utf-16-le" if data[2] or data[3] else "utf-32-le"
    elif len(data) == 2:
        if not data[0]:
            return "utf-16-be"
        if not data[1]:
            return "utf-16-le"
    return "utf-8"


class _TextDecoder:
    # codecs' incremental decoder with errors="replace", for UTF-8 and single-byte encodings
    def __init__(self, encoding):
        self.encoding = encoding
        self.utf8 = encoding.lower().replace("_", "-") in ("utf-8", "utf8")
        self.pending = b""

    def decode(self, data):
        if not self.utf8:
            return data.decode(self.encoding, "replace")
        data = self.pending + data
        cut = len(data)
        i = len(data) - 1
        k = 0
        while i >= 0 and k < 3 and (data[i] & 0xC0) == 0x80:
            i -= 1
            k += 1
        if i >= 0:
            b = data[i]
            need = 2 if b >= 0xC0 and b < 0xE0 else (3 if b >= 0xE0 and b < 0xF0 else (4 if b >= 0xF0 and b < 0xF8 else 1))
            if need > 1 and len(data) - i < need:
                cut = i
        self.pending = data[cut:]
        return data[:cut].decode("utf-8", "replace")

    def flush(self):
        p = self.pending
        self.pending = b""
        return p.decode(self.encoding, "replace") if p else ""


class _Chunker:
    def __init__(self, size, empty):
        self.size = size
        self.buf = empty
        self.empty = empty

    def decode(self, content):
        if self.size is None:
            return [content] if content else []
        self.buf += content
        if len(self.buf) >= self.size:
            v = self.buf
            chunks = [v[i:i + self.size] for i in range(0, len(v), self.size)]
            if len(chunks[-1]) == self.size:
                self.buf = self.empty
                return chunks
            self.buf = chunks[-1]
            return chunks[:-1]
        return []

    def flush(self):
        v = self.buf
        self.buf = self.empty
        return [v] if v else []


_NEWLINES = "\n\r\x0b\x0c\x1c\x1d\x1e\x85  "


class _LineDecoder:
    def __init__(self):
        self.buffer = []
        self.trailing_cr = False

    def decode(self, text):
        if self.trailing_cr:
            text = "\r" + text
            self.trailing_cr = False
        if text.endswith("\r"):
            self.trailing_cr = True
            text = text[:-1]
        if not text:
            return []
        trailing_newline = text[-1] in _NEWLINES
        lines = text.splitlines()
        if len(lines) == 1 and not trailing_newline:
            self.buffer.append(lines[0])
            return []
        if self.buffer:
            lines = ["".join(self.buffer) + lines[0]] + lines[1:]
            self.buffer = []
        if not trailing_newline:
            self.buffer = [lines.pop()]
        return lines

    def flush(self):
        if not self.buffer and not self.trailing_cr:
            return []
        lines = ["".join(self.buffer)]
        self.buffer = []
        self.trailing_cr = False
        return lines


class _ContentDecoder:
    # Content-Encoding gzip / deflate as httpx decodes them (one gzip member; an unknown coding is passed through)
    def __init__(self, names):
        self.ds = []
        for n in names:
            n = n.strip().lower()
            if n == "gzip":
                self.ds.append(_brevi_http.Decoder("gzip", False))
            elif n == "deflate":
                self.ds.append(_brevi_http.Decoder("deflate"))
        self.ds.reverse()

    def decode(self, data):
        for d in self.ds:
            data = d.feed(data)
        return data

    def close(self):
        for d in self.ds:
            d.close()


class Response:
    def __init__(self, status_code, *, headers=None, content=None, text=None, html=None, json=None, stream=None, request=None, extensions=None, history=None, default_encoding="utf-8"):
        self.status_code = status_code
        self.headers = Headers(headers)
        self._request = request
        self.next_request = None
        self.extensions = {} if extensions is None else dict(extensions)
        self.history = [] if history is None else list(history)
        self.is_closed = False
        self.is_stream_consumed = False
        self.default_encoding = default_encoding
        self._num_bytes_downloaded = 0
        self._wire = None
        if stream is None:
            defaults, stream = _encode_response(content, text, html, json)
            for k, v in defaults.items():
                if k.lower() == "transfer-encoding" and "content-length" in self.headers:
                    continue
                self.headers.setdefault(k, v)
            self.stream = stream
            if isinstance(stream, ByteStream):
                self._content = stream._stream
                self.is_stream_consumed = True
                self.is_closed = True
        else:
            self.stream = stream

    @property
    def elapsed(self):
        if not hasattr(self, "_elapsed"):
            raise RuntimeError("'.elapsed' may only be accessed after the response has been read or closed.")
        return self._elapsed

    @elapsed.setter
    def elapsed(self, v):
        self._elapsed = v

    @property
    def request(self):
        if self._request is None:
            raise RuntimeError("The request instance has not been set on this response.")
        return self._request

    @request.setter
    def request(self, value):
        self._request = value

    @property
    def http_version(self):
        v = self.extensions.get("http_version")
        if v is None:
            return "HTTP/1.1"
        return v.decode("ascii", "ignore")

    @property
    def reason_phrase(self):
        v = self.extensions.get("reason_phrase")
        if v is None:
            return codes.get_reason_phrase(self.status_code)
        return v.decode("ascii", "ignore")

    @property
    def url(self):
        return self.request.url

    @property
    def content(self):
        if not hasattr(self, "_content"):
            raise ResponseNotRead()
        return self._content

    @property
    def text(self):
        if not hasattr(self, "_text"):
            content = self.content
            if not content:
                self._text = ""
            else:
                d = _TextDecoder(self.encoding or "utf-8")
                self._text = d.decode(content) + d.flush()
        return self._text

    @property
    def encoding(self):
        if not hasattr(self, "_encoding"):
            enc = self.charset_encoding
            if enc is None or not _known_encoding(enc):
                if isinstance(self.default_encoding, str):
                    enc = self.default_encoding
                elif hasattr(self, "_content"):
                    enc = self.default_encoding(self._content)
            self._encoding = enc or "utf-8"
        return self._encoding

    @encoding.setter
    def encoding(self, value):
        if hasattr(self, "_text"):
            raise ValueError("Setting encoding after `text` has been accessed is not allowed.")
        self._encoding = value

    @property
    def charset_encoding(self):
        ct = self.headers.get("Content-Type")
        if ct is None:
            return None
        return _content_type_charset(ct)

    @property
    def is_informational(self):
        return codes.is_informational(self.status_code)

    @property
    def is_success(self):
        return codes.is_success(self.status_code)

    @property
    def is_redirect(self):
        return codes.is_redirect(self.status_code)

    @property
    def is_client_error(self):
        return codes.is_client_error(self.status_code)

    @property
    def is_server_error(self):
        return codes.is_server_error(self.status_code)

    @property
    def is_error(self):
        return codes.is_error(self.status_code)

    @property
    def has_redirect_location(self):
        return self.status_code in (301, 302, 303, 307, 308) and "Location" in self.headers

    def raise_for_status(self):
        request = self._request
        if request is None:
            raise RuntimeError("Cannot call `raise_for_status` as the request instance has not been set on this response.")
        if self.is_success:
            return self
        kinds = {1: "Informational response", 3: "Redirect response", 4: "Client error", 5: "Server error"}
        kind = kinds.get(self.status_code // 100, "Invalid status code")
        msg = kind + " '" + str(self.status_code) + " " + self.reason_phrase + "' for url '" + str(self.url) + "'\n"
        if self.has_redirect_location:
            msg += "Redirect location: '" + self.headers["location"] + "'\n"
        msg += "For more information check: https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/" + str(self.status_code)
        raise HTTPStatusError(msg, request=request, response=self)

    def json(self, **kwargs):
        c = self.content
        enc = _json_encoding(c)
        s = c.decode(enc, "surrogatepass" if enc != "utf-8" else "strict") if enc != "utf-8-sig" else c[3:].decode("utf-8")
        return jsonlib.loads(s, **kwargs)

    @property
    def cookies(self):
        if not hasattr(self, "_cookies"):
            self._cookies = Cookies()
            self._cookies.extract_cookies(self)
        return self._cookies

    @property
    def links(self):
        header = self.headers.get("link")
        if header is None:
            return {}
        return {(l.get("rel") or l.get("url")): l for l in _parse_header_links(header)}

    @property
    def num_bytes_downloaded(self):
        return self._num_bytes_downloaded

    def __repr__(self):
        return "<Response [" + str(self.status_code) + " " + self.reason_phrase + "]>"

    def _decoder(self):
        if not hasattr(self, "_cdecoder"):
            self._cdecoder = _ContentDecoder(self.headers.get_list("content-encoding", split_commas=True))
        return self._cdecoder

    def _decode(self, raw):
        try:
            return self._decoder().decode(raw)
        except _brevi_http.DecodeFailed as e:
            raise DecodingError(str(e), request=self._request)

    # --- sync ---

    def read(self):
        if not hasattr(self, "_content"):
            self._content = b"".join(self.iter_bytes())
        return self._content

    def iter_bytes(self, chunk_size=None):
        if hasattr(self, "_content"):
            cs = len(self._content) if chunk_size is None else chunk_size
            for i in range(0, len(self._content), max(cs, 1)):
                yield self._content[i:i + cs]
            return
        chunker = _Chunker(chunk_size, b"")
        for raw in self.iter_raw():
            for c in chunker.decode(self._decode(raw)):
                yield c
        for c in chunker.flush():
            yield c

    def iter_text(self, chunk_size=None):
        d = _TextDecoder(self.encoding or "utf-8")
        chunker = _Chunker(chunk_size, "")
        for b in self.iter_bytes():
            for c in chunker.decode(d.decode(b)):
                yield c
        for c in chunker.decode(d.flush()):
            yield c
        for c in chunker.flush():
            yield c

    def iter_lines(self):
        d = _LineDecoder()
        for t in self.iter_text():
            for l in d.decode(t):
                yield l
        for l in d.flush():
            yield l

    def iter_raw(self, chunk_size=None):
        if self.is_stream_consumed:
            raise StreamConsumed()
        if self.is_closed:
            raise StreamClosed()
        if self._wire is None:
            self.is_stream_consumed = True
            for b in self.stream:
                self._num_bytes_downloaded += len(b)
                yield b
            self.close()
            return
        if self._wire.is_async:
            raise RuntimeError("Attempted to call a sync iterator on an async stream.")
        self.is_stream_consumed = True
        self._num_bytes_downloaded = 0
        chunker = _Chunker(chunk_size, b"")
        while True:
            b = self._wire.next_sync(self._request)
            if not b:
                break
            self._num_bytes_downloaded += len(b)
            for c in chunker.decode(b):
                yield c
        for c in chunker.flush():
            yield c
        self.close()

    def close(self):
        if self._wire is not None and self._wire.is_async:
            raise RuntimeError("Attempted to call an sync close on an async stream.")
        if not self.is_closed:
            self.is_closed = True
            self._finish()

    def _finish(self):
        if self._wire is not None:
            self._wire.close()
        if hasattr(self, "_cdecoder"):
            self._cdecoder.close()
        if hasattr(self, "_start"):
            self._elapsed = datetime.timedelta(seconds=time.perf_counter() - self._start)

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()

    # --- async ---

    async def aread(self):
        if not hasattr(self, "_content"):
            parts = []
            async for b in self.aiter_bytes():
                parts.append(b)
            self._content = b"".join(parts)
        return self._content

    async def aiter_bytes(self, chunk_size=None):
        if hasattr(self, "_content"):
            cs = len(self._content) if chunk_size is None else chunk_size
            for i in range(0, len(self._content), max(cs, 1)):
                yield self._content[i:i + cs]
            return
        chunker = _Chunker(chunk_size, b"")
        async for raw in self.aiter_raw():
            for c in chunker.decode(self._decode(raw)):
                yield c
        for c in chunker.flush():
            yield c

    async def aiter_text(self, chunk_size=None):
        d = _TextDecoder(self.encoding or "utf-8")
        chunker = _Chunker(chunk_size, "")
        async for b in self.aiter_bytes():
            for c in chunker.decode(d.decode(b)):
                yield c
        for c in chunker.decode(d.flush()):
            yield c
        for c in chunker.flush():
            yield c

    async def aiter_lines(self):
        d = _LineDecoder()
        async for t in self.aiter_text():
            for l in d.decode(t):
                yield l
        for l in d.flush():
            yield l

    async def aiter_raw(self, chunk_size=None):
        if self.is_stream_consumed:
            raise StreamConsumed()
        if self.is_closed:
            raise StreamClosed()
        if self._wire is None:
            self.is_stream_consumed = True
            if hasattr(self.stream, "__aiter__"):
                async for b in self.stream:
                    self._num_bytes_downloaded += len(b)
                    yield b
            else:
                for b in self.stream:
                    self._num_bytes_downloaded += len(b)
                    yield b
            await self.aclose()
            return
        if not self._wire.is_async:
            raise RuntimeError("Attempted to call an async iterator on an sync stream.")
        self.is_stream_consumed = True
        self._num_bytes_downloaded = 0
        chunker = _Chunker(chunk_size, b"")
        while True:
            b = await self._wire.next_async(self._request)
            if not b:
                break
            self._num_bytes_downloaded += len(b)
            for c in chunker.decode(b):
                yield c
        for c in chunker.flush():
            yield c
        await self.aclose()

    async def aclose(self):
        if self._wire is not None and not self._wire.is_async:
            raise RuntimeError("Attempted to call an async close on an sync stream.")
        if not self.is_closed:
            self.is_closed = True
            self._finish()

    async def __aenter__(self):
        return self

    async def __aexit__(self, *args):
        await self.aclose()


# ---------- the connection pool and the wire (httpcore and h11, cut down) ----------

def _env(name):
    # urllib.request.getproxies_environment: the lowercase name wins
    v = _brevi_http.env_exact(name)
    if v is None:
        v = _brevi_http.env_exact(name.upper())
    return v


def _environment_proxies():
    mounts = {}
    for scheme in ("http", "https", "all"):
        v = _env(scheme + "_proxy")
        if scheme == "http" and _brevi_http.env_exact("REQUEST_METHOD") and _brevi_http.env_exact("http_proxy") is None:
            v = None
        if v:
            mounts[scheme + "://"] = v if "://" in v else "http://" + v
    no = _env("no_proxy") or ""
    for h in [x.strip() for x in no.split(",")]:
        if h == "*":
            return {}
        if h:
            if "://" in h:
                mounts[h] = None
            elif _is_ipv4(h.split("/")[0]):
                mounts["all://" + h] = None
            elif ":" in h and _valid_ipv6(h.split("/")[0]):
                mounts["all://[" + h + "]"] = None
            elif h.lower() == "localhost":
                mounts["all://" + h] = None
            else:
                mounts["all://*" + h] = None
    return mounts


class _URLPattern:
    def __init__(self, pattern):
        if pattern and ":" not in pattern:
            raise ValueError("Proxy keys should use proper URL forms rather than plain scheme strings. Instead of \"" + pattern + "\", use \"" + pattern + "://\"")
        url = URL(pattern)
        self.pattern = pattern
        self.scheme = "" if url.scheme == "all" else url.scheme
        self.host = "" if url.host == "*" else url.host
        self.port = url.port

    def matches(self, other):
        if self.scheme and self.scheme != other.scheme:
            return False
        if self.host:
            h = other.host
            if self.host.startswith("*."):
                d = self.host[2:]
                if not (h.endswith("." + d) and len(h) > len(d) + 1):
                    return False
            elif self.host.startswith("*"):
                d = self.host[1:]
                if not (h == d or h.endswith("." + d)):
                    return False
            elif h != self.host:
                return False
        if self.port is not None and self.port != other.port:
            return False
        return True

    @property
    def priority(self):
        return (0 if self.port is not None else 1, -len(self.host), -len(self.scheme))


def _ssl_settings(verify, trust_env):
    # (verify, cafile, capath, check_hostname) for _brevi_http.connect
    if isinstance(verify, str):
        return (True, verify, "", True)
    if verify is True or verify is None:
        cafile = ""
        capath = ""
        if trust_env:
            cafile = _brevi_http.env_exact("SSL_CERT_FILE") or ""
            capath = _brevi_http.env_exact("SSL_CERT_DIR") or ""
        return (True, cafile, capath, True)
    if verify is False:
        return (False, "", "", False)
    raise TypeError("Brevi's httpx takes verify=True, False or a CA bundle path")


class _Pool:
    def __init__(self, verify, trust_env, proxy, is_async):
        self.tls = _ssl_settings(verify, trust_env)
        self.proxy = proxy
        self.idle = {}
        self.is_async = is_async

    def get(self, key):
        lst = self.idle.get(key)
        while lst:
            c = lst.pop()
            # a connection the server closed while it sat idle shows readable: drop it, as httpcore does
            if _brevi_net.poll([(_brevi_net.fileno(c.id), 1)], 0)[0]:
                c.close()
                continue
            return c
        return None

    def put(self, conn):
        self.idle.setdefault(conn.key, []).append(conn)

    def close(self):
        for lst in self.idle.values():
            for c in lst:
                c.close()
        self.idle = {}


def _t(timeouts, k):
    v = timeouts.get(k)
    return v


def _target(request, pool):
    url = request.url
    scheme = url.scheme
    if scheme not in ("http", "https"):
        if scheme == "":
            raise UnsupportedProtocol("Request URL is missing an 'http://' or 'https://' protocol.", request=request)
        raise UnsupportedProtocol("Request URL has an unsupported protocol '" + scheme + "://'.", request=request)
    host = url.raw_host.decode("ascii")
    if not host:
        raise UnsupportedProtocol("Request URL is missing an 'http://' or 'https://' protocol.", request=request)
    port = url.port if url.port is not None else (443 if scheme == "https" else 80)
    target = url.raw_path.decode("ascii")
    headers = []
    for k, v in request.headers.raw:
        headers.append((k.decode("latin-1"), v.decode("latin-1")))
    proxy_t = None
    p = pool.proxy
    if p is not None:
        if p.url.scheme not in ("http", "https"):
            raise UnsupportedProtocol("Brevi's httpx has no SOCKS proxies", request=request)
        auth = None
        if p.auth:
            auth = "Basic " + base64.b64encode(p.raw_auth[0] + b":" + p.raw_auth[1]).decode()
        proxy_t = (p.url.raw_host.decode("ascii"), p.url.port or (443 if p.url.scheme == "https" else 80), auth)
        if scheme == "http":
            target = str(url).split("#")[0]
            if auth is not None:
                headers.append(("Proxy-Authorization", auth))
            for k, v in p.headers.multi_items():
                headers.append((k, v))
    return scheme, host, port, target, headers, proxy_t


def _map_connect_sync(e, request, proxy_t):
    name = type(e).__name__
    if isinstance(e, _brevi_http.Timeout) or name == "TimeoutError":
        return ConnectTimeout("timed out", request=request)
    if proxy_t is not None and name not in ("SSLError", "SSLCertVerificationError", "gaierror", "PermissionError"):
        return ProxyError(str(e), request=request)
    return ConnectError(str(e), request=request)


def _map_connect_async(e, request, proxy_t):
    name = type(e).__name__
    if isinstance(e, _brevi_http.Timeout) or name == "TimeoutError":
        return ConnectTimeout("", request=request)
    if name in ("SSLError", "SSLCertVerificationError", "gaierror", "PermissionError"):
        return ConnectError(str(e), request=request)
    if proxy_t is not None:
        return ProxyError("All connection attempts failed", request=request)
    return ConnectError("All connection attempts failed", request=request)


class _Wire:
    # one response's body as it comes off one connection
    def __init__(self, pool, conn, body, frame, is_async, read_t):
        self.pool = pool
        self.conn = conn
        self.body = body
        self.frame = frame
        self.is_async = is_async
        self.read_t = read_t
        self.done = body.done
        if self.done:
            self._release()

    def _release(self):
        c = self.conn
        if c is None:
            return
        self.conn = None
        if self.body.clean and c.reusable and self.pool is not None:
            self.pool.put(c)
        else:
            c.close()

    def _incomplete(self, request):
        if self.frame[0] == "length":
            got = self.frame[1] - self.body.left
            return RemoteProtocolError("peer closed connection without sending complete message body (received " + str(got) + " bytes, expected " + str(self.frame[1]) + ")", request=request)
        return RemoteProtocolError("peer closed connection without sending complete message body (incomplete chunked read)", request=request)

    def _fail(self):
        if self.conn is not None:
            self.conn.close()
            self.conn = None
        self.done = True

    def next_sync(self, request):
        if self.done:
            return b""
        try:
            b = self.body.next_sync()
        except _brevi_http.Closed:
            self._fail()
            raise self._incomplete(request)
        except _brevi_http.Protocol as e:
            self._fail()
            raise RemoteProtocolError(str(e), request=request)
        except TimeoutError:
            self._fail()
            raise ReadTimeout("timed out", request=request)
        except OSError as e:
            self._fail()
            raise ReadError(str(e), request=request)
        if self.body.done:
            self.done = True
            self._release()
        return b

    async def next_async(self, request):
        if self.done:
            return b""
        try:
            b = await self.body.next_async(_wait, self.read_t)
        except _brevi_http.Closed:
            self._fail()
            raise self._incomplete(request)
        except _brevi_http.Protocol as e:
            self._fail()
            raise RemoteProtocolError(str(e), request=request)
        except _brevi_http.Timeout:
            self._fail()
            raise ReadTimeout("", request=request)
        except OSError as e:
            self._fail()
            raise ReadError(str(e), request=request)
        except BaseException:
            # cancelled while waiting: the connection is left mid-response
            self._fail()
            raise
        if self.body.done:
            self.done = True
            self._release()
        return b

    def close(self):
        if not self.done:
            self._fail()
        else:
            self._release()


def _settimeout(cid, t):
    _brevi_net.settimeout(cid, -1.0 if t is None else float(t))


def _build_response(request, pool, conn, status, reason, version, raw_headers, is_async, read_t):
    resp = Response(status, headers=raw_headers, stream=_WIRE_MARK, extensions={"http_version": version.encode("ascii"), "reason_phrase": reason.encode("latin-1"), "network_stream": None})
    frame = _brevi_http.framing(request.method, status, raw_headers)
    body = _brevi_http.Body(conn.id, frame)
    conn.reusable = frame[0] != "close" and resp.headers.get("connection", "").lower() != "close" and version != "HTTP/1.0" and request.headers.get("connection", "").lower() != "close"
    resp._wire = _Wire(pool, conn, body, frame, is_async, read_t)
    resp.stream = resp._wire
    resp._request = request
    return resp


class _WireMark:
    pass


_WIRE_MARK = _WireMark()


def _body_pieces_sync(stream):
    if isinstance(stream, ByteStream):
        return None, stream._stream
    if isinstance(stream, _IterStream):
        if stream._used:
            raise StreamConsumed()
        stream._used = True
        if stream._async:
            raise RuntimeError("Attempted to send an async request with a sync Client instance.")
        return stream._it, None
    return None, b"".join(stream)


def _send_sync(pool, request):
    timeouts = request.extensions.get("timeout") or DEFAULT_TIMEOUT_CONFIG.as_dict()
    scheme, host, port, target, headers, proxy_t = _target(request, pool)
    key = (scheme, host, port, pool.tls, proxy_t)
    head = _brevi_http.request_bytes(request.method, target, headers)
    it, whole = _body_pieces_sync(request.stream)
    for attempt in range(2):
        conn = pool.get(key)
        fresh = conn is None
        if fresh:
            try:
                cid = _brevi_http.connect(scheme, host, port, _t(timeouts, "connect"), pool.tls, proxy_t)
            except (OSError, _brevi_http.Timeout, _brevi_http.Closed) as e:
                raise _map_connect_sync(e, request, proxy_t)
            conn = _brevi_http.Conn(cid, key)
        try:
            _settimeout(conn.id, _t(timeouts, "write"))
            if it is None:
                _brevi_http.send_sync(conn.id, head + whole)
            else:
                _brevi_http.send_sync(conn.id, head)
                for piece in it:
                    piece = _to_bytes(piece)
                    if piece:
                        _brevi_http.send_sync(conn.id, ("%x\r\n" % len(piece)).encode() + piece + b"\r\n")
                _brevi_http.send_sync(conn.id, b"0\r\n\r\n")
            _settimeout(conn.id, _t(timeouts, "read"))
            status, reason, version, raw_headers = _brevi_http.read_head(conn.id)
            break
        except TimeoutError:
            conn.close()
            raise ReadTimeout("timed out", request=request)
        except (_brevi_http.Closed, ConnectionResetError, BrokenPipeError) as e:
            conn.close()
            if not fresh and it is None:
                continue
            if isinstance(e, _brevi_http.Closed):
                raise RemoteProtocolError("Server disconnected without sending a response.", request=request)
            raise ReadError(str(e), request=request)
        except _brevi_http.Protocol as e:
            conn.close()
            raise RemoteProtocolError("illegal status line: " + repr(e.args[0] if e.args else b""), request=request)
        except OSError as e:
            conn.close()
            raise ReadError(str(e), request=request)
    return _build_response(request, pool, conn, status, reason, version, raw_headers, False, _t(timeouts, "read"))


# --- async ---

async def _wait(cid, events, timeout):
    # wait until the socket is readable (1) or writable (4), at most timeout seconds (None: no limit)
    loop = asyncio.get_running_loop()
    if loop._io_poll is None:
        loop._io_poll = _brevi_net.poll
    fut = loop._brevi_wait_fd(_brevi_net.fileno(cid), events)
    if timeout is None:
        await fut
        return
    try:
        await asyncio.wait_for(fut, timeout)
    except TimeoutError:
        raise _brevi_http.Timeout("wait", "")


async def _send_async(pool, request):
    timeouts = request.extensions.get("timeout") or DEFAULT_TIMEOUT_CONFIG.as_dict()
    scheme, host, port, target, headers, proxy_t = _target(request, pool)
    key = (scheme, host, port, pool.tls, proxy_t)
    head = _brevi_http.request_bytes(request.method, target, headers)
    stream = request.stream
    it = None
    whole = None
    if isinstance(stream, ByteStream):
        whole = stream._stream
    elif isinstance(stream, _IterStream):
        if stream._used:
            raise StreamConsumed()
        stream._used = True
        it = stream
    else:
        whole = b"".join(stream)
    write_t = _t(timeouts, "write")
    read_t = _t(timeouts, "read")
    for attempt in range(2):
        conn = pool.get(key)
        fresh = conn is None
        if fresh:
            try:
                cid = await _brevi_http.connect_async(scheme, host, port, pool.tls, _wait, _t(timeouts, "connect"), proxy_t)
            except (OSError, _brevi_http.Timeout, _brevi_http.Closed) as e:
                raise _map_connect_async(e, request, proxy_t)
            conn = _brevi_http.Conn(cid, key)
        try:
            if it is None:
                await _brevi_http.send_async(conn.id, head + whole, _wait, write_t)
            else:
                await _brevi_http.send_async(conn.id, head, _wait, write_t)
                if it._async:
                    async for piece in it._it:
                        piece = _to_bytes(piece)
                        if piece:
                            await _brevi_http.send_async(conn.id, ("%x\r\n" % len(piece)).encode() + piece + b"\r\n", _wait, write_t)
                else:
                    for piece in it._it:
                        piece = _to_bytes(piece)
                        if piece:
                            await _brevi_http.send_async(conn.id, ("%x\r\n" % len(piece)).encode() + piece + b"\r\n", _wait, write_t)
                await _brevi_http.send_async(conn.id, b"0\r\n\r\n", _wait, write_t)
            status, reason, version, raw_headers = await _brevi_http.read_head_async(conn.id, _wait, read_t)
            break
        except _brevi_http.Timeout:
            conn.close()
            raise ReadTimeout("", request=request)
        except (_brevi_http.Closed, ConnectionResetError, BrokenPipeError) as e:
            conn.close()
            if not fresh and it is None:
                continue
            if isinstance(e, _brevi_http.Closed):
                raise RemoteProtocolError("Server disconnected without sending a response.", request=request)
            raise ReadError(str(e), request=request)
        except _brevi_http.Protocol as e:
            conn.close()
            raise RemoteProtocolError("illegal status line: " + repr(e.args[0] if e.args else b""), request=request)
        except OSError as e:
            conn.close()
            raise ReadError(str(e), request=request)
        except BaseException:
            conn.close()
            raise
    return _build_response(request, pool, conn, status, reason, version, raw_headers, True, read_t)


# ---------- clients ----------

def _is_https_redirect(url, location):
    if url.host != location.host:
        return False
    return url.scheme == "http" and _port_or_default(url) == 80 and location.scheme == "https" and _port_or_default(location) == 443


def _port_or_default(url):
    if url.port is not None:
        return url.port
    return {"http": 80, "https": 443}.get(url.scheme)


def _same_origin(url, other):
    return url.scheme == other.scheme and url.host == other.host and _port_or_default(url) == _port_or_default(other)


class _BaseClient:
    def __init__(self, auth, params, headers, cookies, timeout, follow_redirects, max_redirects, event_hooks, base_url, trust_env, default_encoding):
        event_hooks = {} if event_hooks is None else event_hooks
        self._base_url = self._enforce_trailing_slash(URL(base_url))
        self._auth = self._build_auth(auth)
        self._params = QueryParams(params)
        self.headers = headers
        self._cookies = Cookies(cookies)
        self._timeout = Timeout(timeout)
        self.follow_redirects = follow_redirects
        self.max_redirects = max_redirects
        self._event_hooks = {"request": list(event_hooks.get("request", [])), "response": list(event_hooks.get("response", []))}
        self._trust_env = trust_env
        self._default_encoding = default_encoding
        self._state = 1

    def _setup_pools(self, verify, trust_env, proxy, is_async):
        self._pool = _Pool(verify, trust_env, None, is_async)
        mounts = []
        if proxy is None:
            if trust_env:
                for k, v in _environment_proxies().items():
                    mounts.append((_URLPattern(k), None if v is None else _Pool(verify, trust_env, Proxy(url=v), is_async)))
        else:
            p = Proxy(url=proxy) if isinstance(proxy, (str, URL)) else proxy
            mounts.append((_URLPattern("all://"), _Pool(verify, trust_env, p, is_async)))
        mounts.sort(key=lambda m: m[0].priority)
        self._mounts = mounts

    def _pool_for(self, url):
        for pat, pool in self._mounts:
            if pat.matches(url):
                return self._pool if pool is None else pool
        return self._pool

    def _close_pools(self):
        self._pool.close()
        for _, p in self._mounts:
            if p is not None:
                p.close()

    @property
    def is_closed(self):
        return self._state == 3

    @property
    def trust_env(self):
        return self._trust_env

    def _enforce_trailing_slash(self, url):
        if url.raw_path.endswith(b"/"):
            return url
        return url.copy_with(raw_path=url.raw_path + b"/")

    @property
    def timeout(self):
        return self._timeout

    @timeout.setter
    def timeout(self, t):
        self._timeout = Timeout(t)

    @property
    def event_hooks(self):
        return self._event_hooks

    @event_hooks.setter
    def event_hooks(self, hooks):
        self._event_hooks = {"request": list(hooks.get("request", [])), "response": list(hooks.get("response", []))}

    @property
    def auth(self):
        return self._auth

    @auth.setter
    def auth(self, auth):
        self._auth = self._build_auth(auth)

    @property
    def base_url(self):
        return self._base_url

    @base_url.setter
    def base_url(self, url):
        self._base_url = self._enforce_trailing_slash(URL(url))

    @property
    def headers(self):
        return self._headers

    @headers.setter
    def headers(self, headers):
        h = Headers({b"Accept": b"*/*", b"Accept-Encoding": ACCEPT_ENCODING.encode("ascii"), b"Connection": b"keep-alive", b"User-Agent": USER_AGENT.encode("ascii")})
        h.update(headers)
        self._headers = h

    @property
    def cookies(self):
        return self._cookies

    @cookies.setter
    def cookies(self, cookies):
        self._cookies = Cookies(cookies)

    @property
    def params(self):
        return self._params

    @params.setter
    def params(self, params):
        self._params = QueryParams(params)

    def build_request(self, method, url, *, content=None, data=None, files=None, json=None, params=None, headers=None, cookies=None, timeout=USE_CLIENT_DEFAULT, extensions=None):
        url = self._merge_url(url)
        headers = self._merge_headers(headers)
        cookies = self._merge_cookies(cookies)
        params = self._merge_queryparams(params)
        extensions = {} if extensions is None else extensions
        if "timeout" not in extensions:
            t = self.timeout if isinstance(timeout, UseClientDefault) else Timeout(timeout)
            extensions = dict(extensions)
            extensions["timeout"] = t.as_dict()
        return Request(method, url, content=content, data=data, files=files, json=json, params=params, headers=headers, cookies=cookies, extensions=extensions)

    def _merge_url(self, url):
        m = URL(url)
        if m.is_relative_url:
            return self.base_url.copy_with(raw_path=self.base_url.raw_path + m.raw_path.lstrip(b"/"))
        return m

    def _merge_cookies(self, cookies=None):
        if cookies or self.cookies:
            c = Cookies(self.cookies)
            c.update(cookies)
            return c
        return cookies

    def _merge_headers(self, headers=None):
        h = Headers(self.headers)
        h.update(headers)
        return h

    def _merge_queryparams(self, params=None):
        if params or self.params:
            return QueryParams(self.params).merge(params)
        return params

    def _build_auth(self, auth):
        if auth is None:
            return None
        if isinstance(auth, tuple):
            return BasicAuth(username=auth[0], password=auth[1])
        if isinstance(auth, Auth):
            return auth
        if callable(auth):
            return FunctionAuth(func=auth)
        raise TypeError('Invalid "auth" argument: ' + repr(auth))

    def _build_request_auth(self, request, auth=USE_CLIENT_DEFAULT):
        auth = self._auth if isinstance(auth, UseClientDefault) else self._build_auth(auth)
        if auth is not None:
            return auth
        u = request.url.username
        p = request.url.password
        if u or p:
            return BasicAuth(username=u, password=p)
        return Auth()

    def _build_redirect_request(self, request, response):
        method = self._redirect_method(request, response)
        url = self._redirect_url(request, response)
        headers = self._redirect_headers(request, url, method)
        stream = self._redirect_stream(request, method)
        return Request(method=method, url=url, headers=headers, cookies=Cookies(self.cookies), stream=stream, extensions=request.extensions)

    def _redirect_method(self, request, response):
        method = request.method
        if response.status_code == 303 and method != "HEAD":
            method = "GET"
        if response.status_code == 302 and method != "HEAD":
            method = "GET"
        if response.status_code == 301 and method == "POST":
            method = "GET"
        return method

    def _redirect_url(self, request, response):
        location = response.headers["Location"]
        try:
            url = URL(location)
        except InvalidURL as exc:
            raise RemoteProtocolError("Invalid URL in location header: " + str(exc) + ".", request=request)
        if url.scheme and not url.host:
            url = url.copy_with(host=request.url.host)
        if url.is_relative_url:
            url = request.url.join(url)
        if request.url.fragment and not url.fragment:
            url = url.copy_with(fragment=request.url.fragment)
        return url

    def _redirect_headers(self, request, url, method):
        headers = Headers(request.headers)
        if not _same_origin(url, request.url):
            if not _is_https_redirect(request.url, url):
                headers.pop("Authorization", None)
            headers["Host"] = url.netloc.decode("ascii")
        if method != request.method and method == "GET":
            headers.pop("Content-Length", None)
            headers.pop("Transfer-Encoding", None)
        headers.pop("Cookie", None)
        return headers

    def _redirect_stream(self, request, method):
        if method != request.method and method == "GET":
            return None
        return request.stream

    def _set_timeout(self, request):
        if "timeout" not in request.extensions:
            ext = dict(request.extensions)
            ext["timeout"] = Timeout(self.timeout).as_dict()
            request.extensions = ext


class _StreamCM:
    def __init__(self, client, args, kwargs, owner=None):
        self.client = client
        self.args = args
        self.kwargs = kwargs
        self.owner = owner
        self.response = None

    def __enter__(self):
        self.response = self.client._stream_send(self.args, self.kwargs)
        return self.response

    def __exit__(self, *exc):
        if self.response is not None:
            self.response.close()
        if self.owner is not None:
            self.owner.close()

    async def __aenter__(self):
        self.response = await self.client._stream_send(self.args, self.kwargs)
        return self.response

    async def __aexit__(self, *exc):
        if self.response is not None:
            await self.response.aclose()


def _split_send_kwargs(kwargs):
    send = {}
    for k in ("auth", "follow_redirects"):
        if k in kwargs:
            send[k] = kwargs.pop(k)
    return send


class Client(_BaseClient):
    def __init__(self, *, auth=None, params=None, headers=None, cookies=None, verify=True, cert=None, trust_env=True, http1=True, http2=False, proxy=None, mounts=None, timeout=DEFAULT_TIMEOUT_CONFIG, follow_redirects=False, limits=DEFAULT_LIMITS, max_redirects=DEFAULT_MAX_REDIRECTS, event_hooks=None, base_url="", transport=None, default_encoding="utf-8"):
        if http2 or cert is not None or mounts is not None or transport is not None:
            raise NotImplementedError("Brevi's httpx has no http2, cert, mounts or transport")
        _BaseClient.__init__(self, auth, params, headers, cookies, timeout, follow_redirects, max_redirects, event_hooks, base_url, trust_env, default_encoding)
        self._setup_pools(verify, trust_env, proxy, False)

    def request(self, method, url, *, content=None, data=None, files=None, json=None, params=None, headers=None, cookies=None, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT, timeout=USE_CLIENT_DEFAULT, extensions=None):
        request = self.build_request(method=method, url=url, content=content, data=data, files=files, json=json, params=params, headers=headers, cookies=cookies, timeout=timeout, extensions=extensions)
        return self.send(request, auth=auth, follow_redirects=follow_redirects)

    def stream(self, method, url, **kwargs):
        return _StreamCM(self, (method, url), kwargs)

    def _stream_send(self, args, kwargs):
        kwargs = dict(kwargs)
        send = _split_send_kwargs(kwargs)
        request = self.build_request(args[0], args[1], **kwargs)
        return self.send(request, stream=True, **send)

    def send(self, request, *, stream=False, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT):
        if self._state == 3:
            raise RuntimeError("Cannot send a request, as the client has been closed.")
        self._state = 2
        follow = self.follow_redirects if isinstance(follow_redirects, UseClientDefault) else follow_redirects
        self._set_timeout(request)
        auth = self._build_request_auth(request, auth)
        response = self._send_handling_auth(request, auth, follow, [])
        try:
            if not stream:
                response.read()
            return response
        except BaseException:
            response.close()
            raise

    def _send_handling_auth(self, request, auth, follow, history):
        if auth.requires_request_body:
            request.read()
        flow = auth.auth_flow(request)
        request = next(flow)
        while True:
            response = self._send_handling_redirects(request, follow, history)
            try:
                if auth.requires_response_body:
                    response.read()
                try:
                    next_request = flow.send(response)
                except StopIteration:
                    return response
                response.history = list(history)
                response.read()
                request = next_request
                history.append(response)
            except BaseException:
                response.close()
                raise

    def _send_handling_redirects(self, request, follow, history):
        while True:
            if len(history) > self.max_redirects:
                raise TooManyRedirects("Exceeded maximum allowed redirects.", request=request)
            for hook in self._event_hooks["request"]:
                hook(request)
            response = self._send_single_request(request)
            try:
                for hook in self._event_hooks["response"]:
                    hook(response)
                response.history = list(history)
                if not response.has_redirect_location:
                    return response
                request = self._build_redirect_request(request, response)
                history = history + [response]
                if follow:
                    response.read()
                else:
                    response.next_request = request
                    return response
            except BaseException:
                response.close()
                raise

    def _send_single_request(self, request):
        pool = self._pool_for(request.url)
        start = time.perf_counter()
        if isinstance(request.stream, _IterStream) and request.stream._async:
            raise RuntimeError("Attempted to send an async request with a sync Client instance.")
        response = _send_sync(pool, request)
        response._start = start
        self.cookies.extract_cookies(response)
        response.default_encoding = self._default_encoding
        return response

    def get(self, url, *, params=None, headers=None, cookies=None, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT, timeout=USE_CLIENT_DEFAULT, extensions=None):
        return self.request("GET", url, params=params, headers=headers, cookies=cookies, auth=auth, follow_redirects=follow_redirects, timeout=timeout, extensions=extensions)

    def options(self, url, *, params=None, headers=None, cookies=None, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT, timeout=USE_CLIENT_DEFAULT, extensions=None):
        return self.request("OPTIONS", url, params=params, headers=headers, cookies=cookies, auth=auth, follow_redirects=follow_redirects, timeout=timeout, extensions=extensions)

    def head(self, url, *, params=None, headers=None, cookies=None, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT, timeout=USE_CLIENT_DEFAULT, extensions=None):
        return self.request("HEAD", url, params=params, headers=headers, cookies=cookies, auth=auth, follow_redirects=follow_redirects, timeout=timeout, extensions=extensions)

    def post(self, url, *, content=None, data=None, files=None, json=None, params=None, headers=None, cookies=None, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT, timeout=USE_CLIENT_DEFAULT, extensions=None):
        return self.request("POST", url, content=content, data=data, files=files, json=json, params=params, headers=headers, cookies=cookies, auth=auth, follow_redirects=follow_redirects, timeout=timeout, extensions=extensions)

    def put(self, url, *, content=None, data=None, files=None, json=None, params=None, headers=None, cookies=None, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT, timeout=USE_CLIENT_DEFAULT, extensions=None):
        return self.request("PUT", url, content=content, data=data, files=files, json=json, params=params, headers=headers, cookies=cookies, auth=auth, follow_redirects=follow_redirects, timeout=timeout, extensions=extensions)

    def patch(self, url, *, content=None, data=None, files=None, json=None, params=None, headers=None, cookies=None, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT, timeout=USE_CLIENT_DEFAULT, extensions=None):
        return self.request("PATCH", url, content=content, data=data, files=files, json=json, params=params, headers=headers, cookies=cookies, auth=auth, follow_redirects=follow_redirects, timeout=timeout, extensions=extensions)

    def delete(self, url, *, params=None, headers=None, cookies=None, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT, timeout=USE_CLIENT_DEFAULT, extensions=None):
        return self.request("DELETE", url, params=params, headers=headers, cookies=cookies, auth=auth, follow_redirects=follow_redirects, timeout=timeout, extensions=extensions)

    def close(self):
        if self._state != 3:
            self._state = 3
            self._close_pools()

    def __enter__(self):
        if self._state != 1:
            msg = {2: "Cannot open a client instance more than once.", 3: "Cannot reopen a client instance, once it has been closed."}[self._state]
            raise RuntimeError(msg)
        self._state = 2
        return self

    def __exit__(self, *args):
        self._state = 3
        self._close_pools()


class AsyncClient(_BaseClient):
    def __init__(self, *, auth=None, params=None, headers=None, cookies=None, verify=True, cert=None, http1=True, http2=False, proxy=None, mounts=None, timeout=DEFAULT_TIMEOUT_CONFIG, follow_redirects=False, limits=DEFAULT_LIMITS, max_redirects=DEFAULT_MAX_REDIRECTS, event_hooks=None, base_url="", transport=None, trust_env=True, default_encoding="utf-8"):
        if http2 or cert is not None or mounts is not None or transport is not None:
            raise NotImplementedError("Brevi's httpx has no http2, cert, mounts or transport")
        _BaseClient.__init__(self, auth, params, headers, cookies, timeout, follow_redirects, max_redirects, event_hooks, base_url, trust_env, default_encoding)
        self._setup_pools(verify, trust_env, proxy, True)

    async def request(self, method, url, *, content=None, data=None, files=None, json=None, params=None, headers=None, cookies=None, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT, timeout=USE_CLIENT_DEFAULT, extensions=None):
        request = self.build_request(method=method, url=url, content=content, data=data, files=files, json=json, params=params, headers=headers, cookies=cookies, timeout=timeout, extensions=extensions)
        return await self.send(request, auth=auth, follow_redirects=follow_redirects)

    def stream(self, method, url, **kwargs):
        return _StreamCM(self, (method, url), kwargs)

    async def _stream_send(self, args, kwargs):
        kwargs = dict(kwargs)
        send = _split_send_kwargs(kwargs)
        request = self.build_request(args[0], args[1], **kwargs)
        return await self.send(request, stream=True, **send)

    async def send(self, request, *, stream=False, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT):
        if self._state == 3:
            raise RuntimeError("Cannot send a request, as the client has been closed.")
        self._state = 2
        follow = self.follow_redirects if isinstance(follow_redirects, UseClientDefault) else follow_redirects
        self._set_timeout(request)
        auth = self._build_request_auth(request, auth)
        response = await self._send_handling_auth(request, auth, follow, [])
        try:
            if not stream:
                await response.aread()
            return response
        except BaseException:
            await response.aclose()
            raise

    async def _send_handling_auth(self, request, auth, follow, history):
        if auth.requires_request_body:
            await request.aread()
        flow = auth.auth_flow(request)
        request = next(flow)
        while True:
            response = await self._send_handling_redirects(request, follow, history)
            try:
                if auth.requires_response_body:
                    await response.aread()
                try:
                    next_request = flow.send(response)
                except StopIteration:
                    return response
                response.history = list(history)
                await response.aread()
                request = next_request
                history.append(response)
            except BaseException:
                await response.aclose()
                raise

    async def _send_handling_redirects(self, request, follow, history):
        while True:
            if len(history) > self.max_redirects:
                raise TooManyRedirects("Exceeded maximum allowed redirects.", request=request)
            for hook in self._event_hooks["request"]:
                await hook(request)
            response = await self._send_single_request(request)
            try:
                for hook in self._event_hooks["response"]:
                    await hook(response)
                response.history = list(history)
                if not response.has_redirect_location:
                    return response
                request = self._build_redirect_request(request, response)
                history = history + [response]
                if follow:
                    await response.aread()
                else:
                    response.next_request = request
                    return response
            except BaseException:
                await response.aclose()
                raise

    async def _send_single_request(self, request):
        pool = self._pool_for(request.url)
        start = time.perf_counter()
        response = await _send_async(pool, request)
        response._start = start
        self.cookies.extract_cookies(response)
        response.default_encoding = self._default_encoding
        return response

    async def get(self, url, *, params=None, headers=None, cookies=None, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT, timeout=USE_CLIENT_DEFAULT, extensions=None):
        return await self.request("GET", url, params=params, headers=headers, cookies=cookies, auth=auth, follow_redirects=follow_redirects, timeout=timeout, extensions=extensions)

    async def options(self, url, *, params=None, headers=None, cookies=None, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT, timeout=USE_CLIENT_DEFAULT, extensions=None):
        return await self.request("OPTIONS", url, params=params, headers=headers, cookies=cookies, auth=auth, follow_redirects=follow_redirects, timeout=timeout, extensions=extensions)

    async def head(self, url, *, params=None, headers=None, cookies=None, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT, timeout=USE_CLIENT_DEFAULT, extensions=None):
        return await self.request("HEAD", url, params=params, headers=headers, cookies=cookies, auth=auth, follow_redirects=follow_redirects, timeout=timeout, extensions=extensions)

    async def post(self, url, *, content=None, data=None, files=None, json=None, params=None, headers=None, cookies=None, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT, timeout=USE_CLIENT_DEFAULT, extensions=None):
        return await self.request("POST", url, content=content, data=data, files=files, json=json, params=params, headers=headers, cookies=cookies, auth=auth, follow_redirects=follow_redirects, timeout=timeout, extensions=extensions)

    async def put(self, url, *, content=None, data=None, files=None, json=None, params=None, headers=None, cookies=None, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT, timeout=USE_CLIENT_DEFAULT, extensions=None):
        return await self.request("PUT", url, content=content, data=data, files=files, json=json, params=params, headers=headers, cookies=cookies, auth=auth, follow_redirects=follow_redirects, timeout=timeout, extensions=extensions)

    async def patch(self, url, *, content=None, data=None, files=None, json=None, params=None, headers=None, cookies=None, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT, timeout=USE_CLIENT_DEFAULT, extensions=None):
        return await self.request("PATCH", url, content=content, data=data, files=files, json=json, params=params, headers=headers, cookies=cookies, auth=auth, follow_redirects=follow_redirects, timeout=timeout, extensions=extensions)

    async def delete(self, url, *, params=None, headers=None, cookies=None, auth=USE_CLIENT_DEFAULT, follow_redirects=USE_CLIENT_DEFAULT, timeout=USE_CLIENT_DEFAULT, extensions=None):
        return await self.request("DELETE", url, params=params, headers=headers, cookies=cookies, auth=auth, follow_redirects=follow_redirects, timeout=timeout, extensions=extensions)

    async def aclose(self):
        if self._state != 3:
            self._state = 3
            self._close_pools()

    async def __aenter__(self):
        if self._state != 1:
            msg = {2: "Cannot open a client instance more than once.", 3: "Cannot reopen a client instance, once it has been closed."}[self._state]
            raise RuntimeError(msg)
        self._state = 2
        return self

    async def __aexit__(self, *args):
        self._state = 3
        self._close_pools()


# ---------- the module functions ----------

def request(method, url, *, params=None, content=None, data=None, files=None, json=None, headers=None, cookies=None, auth=None, proxy=None, timeout=DEFAULT_TIMEOUT_CONFIG, follow_redirects=False, verify=True, trust_env=True):
    with Client(cookies=cookies, proxy=proxy, verify=verify, timeout=timeout, trust_env=trust_env) as client:
        return client.request(method=method, url=url, content=content, data=data, files=files, json=json, params=params, headers=headers, auth=auth, follow_redirects=follow_redirects)


def stream(method, url, *, params=None, content=None, data=None, files=None, json=None, headers=None, cookies=None, auth=None, proxy=None, timeout=DEFAULT_TIMEOUT_CONFIG, follow_redirects=False, verify=True, trust_env=True):
    client = Client(cookies=cookies, proxy=proxy, verify=verify, timeout=timeout, trust_env=trust_env)
    return _StreamCM(client, (method, url), {"params": params, "content": content, "data": data, "files": files, "json": json, "headers": headers, "auth": auth, "follow_redirects": follow_redirects}, owner=client)


def get(url, *, params=None, headers=None, cookies=None, auth=None, proxy=None, follow_redirects=False, verify=True, timeout=DEFAULT_TIMEOUT_CONFIG, trust_env=True):
    return request("GET", url, params=params, headers=headers, cookies=cookies, auth=auth, proxy=proxy, follow_redirects=follow_redirects, verify=verify, timeout=timeout, trust_env=trust_env)


def options(url, *, params=None, headers=None, cookies=None, auth=None, proxy=None, follow_redirects=False, verify=True, timeout=DEFAULT_TIMEOUT_CONFIG, trust_env=True):
    return request("OPTIONS", url, params=params, headers=headers, cookies=cookies, auth=auth, proxy=proxy, follow_redirects=follow_redirects, verify=verify, timeout=timeout, trust_env=trust_env)


def head(url, *, params=None, headers=None, cookies=None, auth=None, proxy=None, follow_redirects=False, verify=True, timeout=DEFAULT_TIMEOUT_CONFIG, trust_env=True):
    return request("HEAD", url, params=params, headers=headers, cookies=cookies, auth=auth, proxy=proxy, follow_redirects=follow_redirects, verify=verify, timeout=timeout, trust_env=trust_env)


def post(url, *, content=None, data=None, files=None, json=None, params=None, headers=None, cookies=None, auth=None, proxy=None, follow_redirects=False, verify=True, timeout=DEFAULT_TIMEOUT_CONFIG, trust_env=True):
    return request("POST", url, content=content, data=data, files=files, json=json, params=params, headers=headers, cookies=cookies, auth=auth, proxy=proxy, follow_redirects=follow_redirects, verify=verify, timeout=timeout, trust_env=trust_env)


def put(url, *, content=None, data=None, files=None, json=None, params=None, headers=None, cookies=None, auth=None, proxy=None, follow_redirects=False, verify=True, timeout=DEFAULT_TIMEOUT_CONFIG, trust_env=True):
    return request("PUT", url, content=content, data=data, files=files, json=json, params=params, headers=headers, cookies=cookies, auth=auth, proxy=proxy, follow_redirects=follow_redirects, verify=verify, timeout=timeout, trust_env=trust_env)


def patch(url, *, content=None, data=None, files=None, json=None, params=None, headers=None, cookies=None, auth=None, proxy=None, follow_redirects=False, verify=True, timeout=DEFAULT_TIMEOUT_CONFIG, trust_env=True):
    return request("PATCH", url, content=content, data=data, files=files, json=json, params=params, headers=headers, cookies=cookies, auth=auth, proxy=proxy, follow_redirects=follow_redirects, verify=verify, timeout=timeout, trust_env=trust_env)


def delete(url, *, params=None, headers=None, cookies=None, auth=None, proxy=None, follow_redirects=False, verify=True, timeout=DEFAULT_TIMEOUT_CONFIG, trust_env=True):
    return request("DELETE", url, params=params, headers=headers, cookies=cookies, auth=auth, proxy=proxy, follow_redirects=follow_redirects, verify=verify, timeout=timeout, trust_env=trust_env)
