# Brevi's requests: the API of requests 2.34 (sessions, the module functions, Response, the exception
# classes, status codes, basic auth, cookies) written over Brevi's own HTTP/1.1 (_brevi_http, native sockets
# and TLS). It follows requests' and urllib3's code for what a program sees: the headers sent and their
# order, URL quoting, redirects, encodings, the JSON decoding and the error messages. caps.rs lists what it has
# (OWN_REQUESTS) and what sends a program to CPython's requests instead (REQUESTS_LACKS: files=, proxies=,
# adapters, hooks, raw, ...). It does not read ~/.netrc, and it guesses a body's encoding (no charset, not
# JSON) as ascii or utf-8 where requests asks charset_normalizer. Ported from requests (Apache-2.0, as Brevi is) and
# urllib3 (MIT: src/dynpy/py/LICENSE-urllib3.txt).
import _brevi_http
import base64
import datetime
import json as complexjson
import time

__version__ = "2.34.2"
__title__ = "requests"

DEFAULT_REDIRECT_LIMIT = 30
CONTENT_CHUNK_SIZE = 10 * 1024
ITER_CHUNK_SIZE = 512


# ---------- structures ----------

class CaseInsensitiveDict:
    def __init__(self, data=None, **kwargs):
        self._store = {}
        if data is None:
            data = {}
        self.update(data, **kwargs)

    def __setitem__(self, key, value):
        self._store[key.lower()] = (key, value)

    def __getitem__(self, key):
        return self._store[key.lower()][1]

    def __delitem__(self, key):
        del self._store[key.lower()]

    def __iter__(self):
        return iter([k for k, v in self._store.values()])

    def __len__(self):
        return len(self._store)

    def __contains__(self, key):
        return isinstance(key, str) and key.lower() in self._store

    def get(self, key, default=None):
        if key.lower() in self._store:
            return self._store[key.lower()][1]
        return default

    def pop(self, key, *default):
        if key.lower() in self._store:
            return self._store.pop(key.lower())[1]
        if default:
            return default[0]
        raise KeyError(key)

    def setdefault(self, key, default=None):
        if key.lower() not in self._store:
            self[key] = default
        return self[key]

    def update(self, other=(), **kwargs):
        if isinstance(other, (dict, CaseInsensitiveDict)):
            for k in other.keys():
                self[k] = other[k]
        else:
            for k, v in other:
                self[k] = v
        for k, v in kwargs.items():
            self[k] = v

    def keys(self):
        return [k for k, v in self._store.values()]

    def values(self):
        return [v for k, v in self._store.values()]

    def items(self):
        return [(k, v) for k, v in self._store.values()]

    def lower_items(self):
        return [(lk, kv[1]) for lk, kv in self._store.items()]

    def __eq__(self, other):
        if isinstance(other, dict):
            other = CaseInsensitiveDict(other)
        elif not isinstance(other, CaseInsensitiveDict):
            return NotImplemented
        return dict(self.lower_items()) == dict(other.lower_items())

    def copy(self):
        return CaseInsensitiveDict(self._store.values())

    def __repr__(self):
        return str(dict(self.items()))


class LookupDict(dict):
    def __init__(self, name=None):
        self.name = name
        dict.__init__(self)

    def __repr__(self):
        return "<lookup '" + str(self.name) + "'>"

    def __getitem__(self, key):
        return getattr(self, key, None)

    def get(self, key, default=None):
        return getattr(self, key, default)


_codes = {
    100: ("continue",), 101: ("switching_protocols",), 102: ("processing", "early-hints"), 103: ("checkpoint",),
    122: ("uri_too_long", "request_uri_too_long"), 200: ("ok", "okay", "all_ok", "all_okay", "all_good", "\\o/", "✓"),
    201: ("created",), 202: ("accepted",), 203: ("non_authoritative_info", "non_authoritative_information"),
    204: ("no_content",), 205: ("reset_content", "reset"), 206: ("partial_content", "partial"),
    207: ("multi_status", "multiple_status", "multi_stati", "multiple_stati"), 208: ("already_reported",),
    226: ("im_used",), 300: ("multiple_choices",), 301: ("moved_permanently", "moved", "\\o-"), 302: ("found",),
    303: ("see_other", "other"), 304: ("not_modified",), 305: ("use_proxy",), 306: ("switch_proxy",),
    307: ("temporary_redirect", "temporary_moved", "temporary"),
    308: ("permanent_redirect", "resume_incomplete", "resume"), 400: ("bad_request", "bad"),
    401: ("unauthorized",), 402: ("payment_required", "payment"), 403: ("forbidden",), 404: ("not_found", "-o-"),
    405: ("method_not_allowed", "not_allowed"), 406: ("not_acceptable",),
    407: ("proxy_authentication_required", "proxy_auth", "proxy_authentication"), 408: ("request_timeout", "timeout"),
    409: ("conflict",), 410: ("gone",), 411: ("length_required",), 412: ("precondition_failed", "precondition"),
    413: ("request_entity_too_large", "content_too_large"), 414: ("request_uri_too_large", "uri_too_long"),
    415: ("unsupported_media_type", "unsupported_media", "media_type"),
    416: ("requested_range_not_satisfiable", "requested_range", "range_not_satisfiable"),
    417: ("expectation_failed",), 418: ("im_a_teapot", "teapot", "i_am_a_teapot"), 421: ("misdirected_request",),
    422: ("unprocessable_entity", "unprocessable", "unprocessable_content"), 423: ("locked",),
    424: ("failed_dependency", "dependency"), 425: ("unordered_collection", "unordered", "too_early"),
    426: ("upgrade_required", "upgrade"), 428: ("precondition_required", "precondition"),
    429: ("too_many_requests", "too_many"), 431: ("header_fields_too_large", "fields_too_large"),
    444: ("no_response", "none"), 449: ("retry_with", "retry"),
    450: ("blocked_by_windows_parental_controls", "parental_controls"),
    451: ("unavailable_for_legal_reasons", "legal_reasons"), 499: ("client_closed_request",),
    500: ("internal_server_error", "server_error", "/o\\", "✗"), 501: ("not_implemented",),
    502: ("bad_gateway",), 503: ("service_unavailable", "unavailable"), 504: ("gateway_timeout",),
    505: ("http_version_not_supported", "http_version"), 506: ("variant_also_negotiates",),
    507: ("insufficient_storage",), 509: ("bandwidth_limit_exceeded", "bandwidth"), 510: ("not_extended",),
    511: ("network_authentication_required", "network_auth", "network_authentication"),
}

codes = LookupDict(name="status_codes")
for _code, _titles in _codes.items():
    for _title in _titles:
        setattr(codes, _title, _code)
        if not _title.startswith(("\\", "/")):
            setattr(codes, _title.upper(), _code)


class _Namespace:
    def __init__(self, name):
        self.__name__ = name

    def __repr__(self):
        return "<module '" + self.__name__ + "' (Brevi)>"


# ---------- exceptions ----------

class RequestException(OSError):
    def __init__(self, *args, **kwargs):
        response = kwargs.pop("response", None)
        self.response = response
        self.request = kwargs.pop("request", None)
        if response is not None and not self.request and hasattr(response, "request"):
            self.request = self.response.request
        OSError.__init__(self, *args)


class InvalidJSONError(RequestException):
    pass


class JSONDecodeError(InvalidJSONError, complexjson.JSONDecodeError):
    def __init__(self, msg, doc, pos, **kwargs):
        # json.JSONDecodeError's message and fields, then requests' own
        lineno = doc.count("\n", 0, pos) + 1
        colno = pos - doc.rfind("\n", 0, pos)
        self.msg = msg
        self.doc = doc
        self.pos = pos
        self.lineno = lineno
        self.colno = colno
        InvalidJSONError.__init__(self, "%s: line %d column %d (char %d)" % (msg, lineno, colno, pos), **kwargs)


class HTTPError(RequestException):
    pass


class ConnectionError(RequestException):
    pass


class ProxyError(ConnectionError):
    pass


class SSLError(ConnectionError):
    pass


class Timeout(RequestException):
    pass


class ConnectTimeout(ConnectionError, Timeout):
    pass


class ReadTimeout(Timeout):
    pass


class URLRequired(RequestException):
    pass


class TooManyRedirects(RequestException):
    pass


class MissingSchema(RequestException, ValueError):
    pass


class InvalidSchema(RequestException, ValueError):
    pass


class InvalidURL(RequestException, ValueError):
    pass


class InvalidHeader(RequestException, ValueError):
    pass


class InvalidProxyURL(InvalidURL):
    pass


class ChunkedEncodingError(RequestException):
    pass


class ContentDecodingError(RequestException):
    pass


class StreamConsumedError(RequestException, TypeError):
    pass


class RetryError(RequestException):
    pass


class UnrewindableBodyError(RequestException):
    pass


exceptions = _Namespace("requests.exceptions")
for _e in [RequestException, InvalidJSONError, JSONDecodeError, HTTPError, ConnectionError, ProxyError, SSLError,
           Timeout, ConnectTimeout, ReadTimeout, URLRequired, TooManyRedirects, MissingSchema, InvalidSchema,
           InvalidURL, InvalidHeader, InvalidProxyURL, ChunkedEncodingError, ContentDecodingError,
           StreamConsumedError, RetryError, UnrewindableBodyError]:
    setattr(exceptions, _e.__name__, _e)
status_codes = _Namespace("requests.status_codes")
status_codes.codes = codes


# ---------- URLs ----------

_ALWAYS_SAFE = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_.-~"
UNRESERVED_SET = frozenset("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")


def quote(s, safe="/"):
    if isinstance(s, str):
        b = s.encode("utf-8")
    else:
        b = s
    ok = _ALWAYS_SAFE + safe
    out = []
    for c in b:
        ch = chr(c)
        if c < 128 and ch in ok:
            out.append(ch)
        else:
            out.append("%" + "%02X" % c)
    return "".join(out)


def quote_plus(s, safe=""):
    if isinstance(s, str) and " " not in s or isinstance(s, bytes) and b" " not in s:
        return quote(s, safe)
    return quote(s, safe + " ").replace(" ", "+")


def unquote(s):
    if "%" not in s:
        return s
    parts = s.split("%")
    out = [parts[0].encode("utf-8")]
    for p in parts[1:]:
        h = p[:2]
        if len(h) == 2 and all(c in "0123456789abcdefABCDEF" for c in h):
            out.append(bytes([int(h, 16)]) + p[2:].encode("utf-8"))
        else:
            out.append(b"%" + p.encode("utf-8"))
    return b"".join(out).decode("utf-8", "replace")


def urlencode(query):
    # urllib.parse.urlencode(query, doseq=True)
    if hasattr(query, "items"):
        query = list(query.items())
    out = []
    for k, v in query:
        if isinstance(k, bytes):
            k = quote_plus(k)
        else:
            k = quote_plus(str(k))
        if isinstance(v, bytes):
            out.append(k + "=" + quote_plus(v))
        elif isinstance(v, str):
            out.append(k + "=" + quote_plus(v))
        else:
            try:
                n = len(v)
            except TypeError:
                out.append(k + "=" + quote_plus(str(v)))
            else:
                for elt in v:
                    if isinstance(elt, bytes):
                        out.append(k + "=" + quote_plus(elt))
                    else:
                        out.append(k + "=" + quote_plus(str(elt)))
    return "&".join(out)


def unquote_unreserved(uri):
    parts = uri.split("%")
    for i in range(1, len(parts)):
        h = parts[i][0:2]
        if len(h) == 2 and h.isalnum():
            try:
                c = chr(int(h, 16))
            except ValueError:
                raise InvalidURL("Invalid percent-escape sequence: '" + h + "'")
            if c in UNRESERVED_SET:
                parts[i] = c + parts[i][2:]
            else:
                parts[i] = "%" + parts[i]
        else:
            parts[i] = "%" + parts[i]
    return "".join(parts)


def requote_uri(uri):
    try:
        return quote(unquote_unreserved(uri), safe="!#$%&'()*+,/:;=?@[]~")
    except InvalidURL:
        return quote(uri, safe="!#$&'()*+,/:;=?@[]~")


class SplitResult:
    def __init__(self, scheme, netloc, path, query, fragment):
        self.scheme = scheme
        self.netloc = netloc
        self.path = path
        self.query = query
        self.fragment = fragment

    @property
    def hostname(self):
        h = self.netloc.rpartition("@")[2]
        if h.startswith("["):
            return h[1:h.find("]")].lower() or None
        h = h.partition(":")[0]
        return h.lower() or None

    @property
    def port(self):
        h = self.netloc.rpartition("@")[2]
        if h.startswith("["):
            rest = h[h.find("]") + 1:]
            p = rest[1:] if rest.startswith(":") else ""
        else:
            p = h.partition(":")[2]
        if p and p.isdigit():
            return int(p)
        return None

    @property
    def username(self):
        if "@" not in self.netloc:
            return None
        return self.netloc.rpartition("@")[0].partition(":")[0]

    @property
    def password(self):
        if "@" not in self.netloc:
            return None
        u = self.netloc.rpartition("@")[0]
        return u.partition(":")[2] if ":" in u else None

    def geturl(self):
        return urlunsplit(self.scheme, self.netloc, self.path, self.query, self.fragment)


def urlsplit(url):
    scheme = ""
    i = url.find(":")
    if i > 0 and url[0].isalpha() and all(c.isalnum() or c in "+-." for c in url[:i]):
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
    return SplitResult(scheme, netloc, url, query, fragment)


def urlunsplit(scheme, netloc, path, query, fragment):
    url = path
    if netloc or (scheme and url[:2] != "//"):
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


def _remove_dot_segments(path):
    segs = path.split("/")
    out = []
    for s in segs:
        if s == ".":
            continue
        if s != "..":
            out.append(s)
        elif out:
            out.pop()
    if path.startswith("/") and (not out or out[0]):
        out.insert(0, "")
    if path.endswith(("/.", "/..")):
        out.append("")
    return "/".join(out)


def urljoin(base, url):
    if not base:
        return url
    if not url:
        return base
    b = urlsplit(base)
    u = urlsplit(url)
    if u.scheme and u.scheme != b.scheme:
        return url
    if u.netloc:
        return urlunsplit(b.scheme, u.netloc, _remove_dot_segments(u.path), u.query, u.fragment)
    if not u.path:
        return urlunsplit(b.scheme, b.netloc, b.path, u.query if u.query else b.query, u.fragment)
    if u.path.startswith("/"):
        path = u.path
    else:
        path = b.path[:b.path.rfind("/") + 1] + u.path if "/" in b.path else "/" + u.path
    return urlunsplit(b.scheme, b.netloc, _remove_dot_segments(path), u.query, u.fragment)


# urllib3's parse_url: the parts of an http(s) URL, the host lowercased and the path's dot segments removed,
# with characters URLs cannot hold percent-encoded
_SUB_DELIM = "!$&'()*+,;="
_UNRESERVED = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-~"
_PATH_CHARS = _UNRESERVED + _SUB_DELIM + ":@/"
_QUERY_CHARS = _UNRESERVED + _SUB_DELIM + ":@/?"


def _encode_invalid(component, allowed):
    if component is None:
        return component
    out = []
    b = component.encode("utf-8")
    i = 0
    n = len(b)
    while i < n:
        c = b[i]
        ch = chr(c)
        if ch == "%" and i + 2 < n + 0 and i + 2 <= n - 1 and chr(b[i + 1]) in "0123456789abcdefABCDEF" and chr(b[i + 2]) in "0123456789abcdefABCDEF":
            out.append("%" + chr(b[i + 1]).upper() + chr(b[i + 2]).upper())
            i += 3
            continue
        if c < 128 and ch in allowed:
            out.append(ch)
        else:
            out.append("%" + "%02X" % c)
        i += 1
    return "".join(out)


class _ParsedURL:
    def __init__(self, scheme, auth, host, port, path, query, fragment):
        self.scheme = scheme
        self.auth = auth
        self.host = host
        self.port = port
        self.path = path
        self.query = query
        self.fragment = fragment


def parse_url(url):
    if not url:
        return _ParsedURL(None, None, None, None, None, None, None)
    source = url
    if "://" not in url and not url.startswith("//"):
        url = "//" + url
    scheme = None
    i = url.find("://")
    if i > 0 and url[0].isalpha() and all(c.isalnum() or c in "+-." for c in url[:i]):
        scheme = url[:i].lower()
        rest = url[i + 1:]
    else:
        rest = url
    p = urlsplit(rest)
    netloc = p.netloc
    auth = None
    if "@" in netloc:
        auth, _, hostport = netloc.rpartition("@")
    else:
        hostport = netloc
    port = None
    if hostport.startswith("["):
        k = hostport.find("]")
        host = hostport[:k + 1]
        rest2 = hostport[k + 1:]
        if rest2.startswith(":"):
            port = rest2[1:]
    else:
        host, sep, port_s = hostport.partition(":")
        if sep:
            port = port_s
    if port is not None:
        if port == "":
            port = None
        elif port.isdigit() and int(port) <= 65535:
            port = int(port)
        else:
            raise _LocationParseError(source)
    if not host:
        host = None
    elif scheme in ("http", "https", None):
        host = host.lower()
    path = p.path
    if path and scheme in ("http", "https", None):
        path = _remove_dot_segments(path)
    path = _encode_invalid(path, _PATH_CHARS) if path else (path or None)
    query = _encode_invalid(p.query, _QUERY_CHARS) if "?" in rest else None
    fragment = _encode_invalid(p.fragment, _QUERY_CHARS) if "#" in rest else None
    if auth is not None:
        auth = _encode_invalid(auth, _UNRESERVED + _SUB_DELIM + ":")
    return _ParsedURL(scheme, auth, host, port, path or None, query, fragment)


class _LocationParseError(Exception):
    def __init__(self, location):
        Exception.__init__(self, "Failed to parse: " + location)


def get_auth_from_url(url):
    p = urlsplit(url)
    try:
        return (unquote(p.username), unquote(p.password))
    except (AttributeError, TypeError):
        return ("", "")


def to_key_val_list(value):
    if value is None:
        return None
    if isinstance(value, (str, bytes, bool, int)):
        raise ValueError("cannot encode objects that are not 2-tuples")
    if isinstance(value, (dict, CaseInsensitiveDict)):
        value = value.items()
    return list(value)


def _encode_params(data):
    if isinstance(data, (str, bytes)):
        return data
    elif hasattr(data, "read"):
        return data
    elif hasattr(data, "__iter__"):
        result = []
        for k, vs in to_key_val_list(data):
            if isinstance(vs, (str, bytes)) or not hasattr(vs, "__iter__"):
                vs = [vs]
            for v in vs:
                if v is not None:
                    result.append((k.encode("utf-8") if isinstance(k, str) else k, v.encode("utf-8") if isinstance(v, str) else v))
        return urlencode(result)
    return data


def merge_setting(request_setting, session_setting, dict_class=None):
    if session_setting is None:
        return request_setting
    if request_setting is None:
        return session_setting
    if not (isinstance(session_setting, (dict, CaseInsensitiveDict)) and isinstance(request_setting, (dict, CaseInsensitiveDict))):
        return request_setting
    merged = CaseInsensitiveDict(to_key_val_list(session_setting)) if dict_class is CaseInsensitiveDict else dict(to_key_val_list(session_setting))
    merged.update(to_key_val_list(request_setting))
    none_keys = [k for (k, v) in merged.items() if v is None]
    for key in none_keys:
        del merged[key]
    return merged


def _parse_content_type_header(header):
    tokens = header.split(";")
    content_type, params = tokens[0].strip(), tokens[1:]
    params_dict = {}
    items_to_strip = "\"' "
    for param in params:
        param = param.strip()
        if param:
            key, value = param, True
            index_of_equals = param.find("=")
            if index_of_equals != -1:
                key = param[:index_of_equals].strip(items_to_strip)
                value = param[index_of_equals + 1:].strip(items_to_strip)
            params_dict[key.lower()] = value
    return content_type, params_dict


def get_encoding_from_headers(headers):
    content_type = headers.get("content-type")
    if not content_type:
        return None
    content_type, params = _parse_content_type_header(content_type)
    if "charset" in params:
        return params["charset"].strip("'\"")
    if "text" in content_type:
        return "ISO-8859-1"
    if "application/json" in content_type:
        return "utf-8"
    return None


def guess_json_utf(data):
    sample = data[:4]
    if sample in (b"\xff\xfe\x00\x00", b"\x00\x00\xfe\xff"):
        return "utf-32"
    if sample[:3] == b"\xef\xbb\xbf":
        return "utf-8-sig"
    if sample[:2] in (b"\xff\xfe", b"\xfe\xff"):
        return "utf-16"
    nullcount = sample.count(b"\x00")
    if nullcount == 0:
        return "utf-8"
    if nullcount == 2:
        if sample[0] == 0 and sample[2] == 0:
            return "utf-16-be"
        if sample[1] == 0 and sample[3] == 0:
            return "utf-16-le"
    if nullcount == 3:
        if sample[:3] == b"\x00\x00\x00":
            return "utf-32-be"
        if sample[1:] == b"\x00\x00\x00":
            return "utf-32-le"
    return None


def parse_header_links(value):
    links = []
    replace_chars = " '\""
    value = value.strip(replace_chars)
    if not value:
        return links
    import re
    for val in re.split(", *<", value):
        if ";" in val:
            url, params = val.split(";", 1)
        else:
            url, params = val, ""
        link = {"url": url.strip("<> '\"")}
        for param in params.split(";"):
            if param.count("=") != 1:
                break
            key, v = param.split("=")
            link[key.strip(replace_chars)] = v.strip(replace_chars)
        links.append(link)
    return links


def default_user_agent(name="python-requests"):
    return name + "/" + __version__


def default_headers():
    return CaseInsensitiveDict({"User-Agent": default_user_agent(), "Accept-Encoding": "gzip, deflate", "Accept": "*/*", "Connection": "keep-alive"})


def super_len(o):
    if o is None:
        return 0
    if isinstance(o, str):
        return len(o.encode("utf-8"))
    if isinstance(o, bytes):
        return len(o)
    if hasattr(o, "__len__"):
        return len(o)
    return 0


def iter_slices(string, slice_length):
    pos = 0
    if slice_length is None or slice_length <= 0:
        slice_length = len(string)
    while pos < len(string):
        yield string[pos:pos + slice_length]
        pos += slice_length


class _Utf8Stream:
    # codecs' incremental decoder for UTF-8 with errors="replace": keeps a sequence cut at the end of a chunk
    def __init__(self, encoding):
        self.encoding = encoding
        self.pending = b""

    def decode(self, data, final=False):
        data = self.pending + data
        self.pending = b""
        if self.encoding.lower().replace("_", "-") not in ("utf-8", "utf8"):
            return data.decode(self.encoding, "replace")
        if not final:
            k = len(data)
            for back in range(1, min(4, k) + 1):
                c = data[k - back]
                if c & 0xC0 == 0x80:
                    continue
                need = 2 if c & 0xE0 == 0xC0 else 3 if c & 0xF0 == 0xE0 else 4 if c & 0xF8 == 0xF0 else 1
                if need > back:
                    self.pending = data[k - back:]
                    data = data[:k - back]
                break
        return data.decode("utf-8", "replace")


def stream_decode_response_unicode(iterator, r):
    if r.encoding is None:
        yield from iterator
        return
    dec = _Utf8Stream(r.encoding)
    for chunk in iterator:
        rv = dec.decode(chunk)
        if rv:
            yield rv
    rv = dec.decode(b"", final=True)
    if rv:
        yield rv


def _apparent_encoding(content):
    # what charset_normalizer answers for the bodies it is sure about
    try:
        content.decode("ascii")
        return "ascii"
    except UnicodeDecodeError:
        pass
    try:
        content.decode("utf-8")
        return "utf-8"
    except UnicodeDecodeError:
        return "cp1252"


# ---------- proxies from the environment (urllib.request.getproxies, requests.utils) ----------

def _env_proxies():
    out = {}
    for scheme in ("http", "https", "all"):
        v = _brevi_http.env_exact(scheme + "_proxy")
        if v is None and not (scheme == "http" and _brevi_http.env_exact("REQUEST_METHOD")):
            v = _brevi_http.env_exact(scheme.upper() + "_PROXY")
        if v:
            out[scheme] = v
    return out


def _bypass(host, port, no_proxy):
    if no_proxy is None:
        no_proxy = _brevi_http.env_exact("no_proxy") or _brevi_http.env_exact("NO_PROXY")
    if not no_proxy:
        return False
    if no_proxy == "*":
        return True
    hostonly = host
    entries = [x.strip() for x in no_proxy.replace(" ", "").split(",") if x]
    ipv4 = _is_ipv4(host)
    for e in entries:
        if ipv4 and "/" in e and _is_cidr(e):
            if _in_network(host, e):
                return True
        elif ipv4 and host == e:
            return True
    hp = host if port is None else host + ":" + str(port)
    for e in entries:
        if not e:
            continue
        if hostonly.endswith(e) or hp.endswith(e):
            return True
        if e.startswith(".") is False and (hostonly == e or hostonly.endswith("." + e.lstrip("."))):
            return True
    return False


def _is_ipv4(s):
    parts = s.split(".")
    return len(parts) == 4 and all(p.isdigit() and int(p) < 256 for p in parts)


def _is_cidr(s):
    if s.count("/") != 1:
        return False
    a, m = s.split("/")
    return _is_ipv4(a) and m.isdigit() and 1 <= int(m) <= 32


def _ip_int(s):
    n = 0
    for p in s.split("."):
        n = n * 256 + int(p)
    return n


def _in_network(ip, net):
    a, m = net.split("/")
    mask = (0xFFFFFFFF << (32 - int(m))) & 0xFFFFFFFF
    return (_ip_int(ip) & mask) == (_ip_int(a) & mask)


def select_proxy(url, proxies):
    proxies = proxies or {}
    p = urlsplit(url)
    if p.hostname is None:
        return proxies.get(p.scheme, proxies.get("all"))
    keys = [p.scheme + "://" + p.hostname, p.scheme, "all://" + p.hostname, "all"]
    for k in keys:
        if k in proxies:
            return proxies[k]
    return None


def resolve_proxies(url, proxies, trust_env):
    proxies = dict(proxies) if proxies is not None else {}
    p = urlsplit(url)
    scheme = p.scheme
    no_proxy = proxies.get("no_proxy")
    new = dict(proxies)
    if trust_env and not _bypass(p.hostname or "", p.port, no_proxy):
        env = _env_proxies()
        proxy = env.get(scheme, env.get("all"))
        if proxy:
            new.setdefault(scheme, proxy)
    return new


def _basic_auth_str(username, password):
    if isinstance(username, str):
        username = username.encode("latin1")
    if isinstance(password, str):
        password = password.encode("latin1")
    return "Basic " + base64.b64encode(b":".join((username, password))).strip().decode("ascii")


# ---------- auth ----------

class AuthBase:
    def __call__(self, r):
        raise NotImplementedError("Auth hooks must be callable.")


class HTTPBasicAuth(AuthBase):
    def __init__(self, username, password):
        self.username = username
        self.password = password

    def __eq__(self, other):
        return self.username == getattr(other, "username", None) and self.password == getattr(other, "password", None)

    def __ne__(self, other):
        return not self == other

    def __call__(self, r):
        r.headers["Authorization"] = _basic_auth_str(self.username, self.password)
        return r


class HTTPProxyAuth(HTTPBasicAuth):
    def __call__(self, r):
        r.headers["Proxy-Authorization"] = _basic_auth_str(self.username, self.password)
        return r


auth = _Namespace("requests.auth")
auth.AuthBase = AuthBase
auth.HTTPBasicAuth = HTTPBasicAuth
auth.HTTPProxyAuth = HTTPProxyAuth


# ---------- cookies ----------

class _Cookie:
    def __init__(self, name, value, domain, path, host_only, secure):
        self.name = name
        self.value = value
        self.domain = domain
        self.path = path
        self.host_only = host_only
        self.secure = secure

    def __repr__(self):
        return "Cookie(version=0, name=" + repr(self.name) + ", value=" + repr(self.value) + ", domain=" + repr(self.domain) + ", path=" + repr(self.path) + ")"


class RequestsCookieJar:
    def __init__(self):
        self._cookies = []

    def set(self, name, value, domain="", path="/"):
        self._cookies = [c for c in self._cookies if not (c.name == name and c.domain == domain and c.path == path)]
        if value is not None:
            self._cookies.append(_Cookie(name, value, domain, path, False, False))

    def set_cookie(self, c):
        self._cookies = [x for x in self._cookies if not (x.name == c.name and x.domain == c.domain and x.path == c.path)]
        self._cookies.append(c)

    def get(self, name, default=None, domain=None, path=None):
        for c in self._cookies:
            if c.name == name and (domain is None or c.domain == domain) and (path is None or c.path == path):
                return c.value
        return default

    def __getitem__(self, name):
        for c in self._cookies:
            if c.name == name:
                return c.value
        raise KeyError("name=" + repr(name) + ", domain=None, path=None")

    def __setitem__(self, name, value):
        self.set(name, value)

    def __delitem__(self, name):
        self._cookies = [c for c in self._cookies if c.name != name]

    def __contains__(self, name):
        return any(c.name == name for c in self._cookies)

    def __iter__(self):
        return iter(list(self._cookies))

    def __len__(self):
        return len(self._cookies)

    def keys(self):
        return [c.name for c in self._cookies]

    def values(self):
        return [c.value for c in self._cookies]

    def items(self):
        return [(c.name, c.value) for c in self._cookies]

    def get_dict(self, domain=None, path=None):
        d = {}
        for c in self._cookies:
            if (domain is None or c.domain == domain) and (path is None or c.path == path):
                d[c.name] = c.value
        return d

    def update(self, other):
        if isinstance(other, RequestsCookieJar):
            for c in other._cookies:
                self.set_cookie(c)
        else:
            for k, v in other.items():
                self.set(k, v)

    def clear(self):
        self._cookies = []

    def copy(self):
        j = RequestsCookieJar()
        j._cookies = list(self._cookies)
        return j

    def __repr__(self):
        return "<RequestsCookieJar[" + ", ".join("<Cookie " + c.name + "=" + c.value + " for " + c.domain + c.path + ">" for c in self._cookies) + "]>"

    def _header_for(self, host, path, secure):
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

    def _extract(self, host, path, headers):
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
            c = _Cookie(name.strip(), value.strip(), host if host_only else domain, cpath, host_only, secure)
            if expired:
                self._cookies = [x for x in self._cookies if not (x.name == c.name and x.domain == c.domain and x.path == c.path)]
            else:
                self.set_cookie(c)


def cookiejar_from_dict(cookie_dict, cookiejar=None):
    if cookiejar is None:
        cookiejar = RequestsCookieJar()
    if cookie_dict is not None:
        if isinstance(cookie_dict, RequestsCookieJar):
            cookiejar.update(cookie_dict)
        else:
            for name in cookie_dict:
                cookiejar.set(name, cookie_dict[name])
    return cookiejar


# ---------- models ----------

class Request:
    def __init__(self, method=None, url=None, headers=None, files=None, data=None, params=None, auth=None, cookies=None, hooks=None, json=None):
        self.method = method
        self.url = url
        self.headers = headers if headers is not None else {}
        self.files = files if files is not None else []
        self.data = data if data is not None else []
        self.json = json
        self.params = params if params is not None else {}
        self.auth = auth
        self.cookies = cookies

    def __repr__(self):
        return "<Request [" + str(self.method) + "]>"


class PreparedRequest:
    def __init__(self):
        self.method = None
        self.url = None
        self.headers = None
        self._cookies = None
        self.body = None
        self.hooks = {"response": []}
        self._body_position = None

    def __repr__(self):
        return "<PreparedRequest [" + str(self.method) + "]>"

    def copy(self):
        p = PreparedRequest()
        p.method = self.method
        p.url = self.url
        p.headers = self.headers.copy() if self.headers is not None else None
        p._cookies = self._cookies.copy() if self._cookies is not None else None
        p.body = self.body
        p._body_position = self._body_position
        return p

    def prepare(self, method=None, url=None, headers=None, files=None, data=None, params=None, auth=None, cookies=None, hooks=None, json=None):
        self.prepare_method(method)
        self.prepare_url(url, params)
        self.prepare_headers(headers)
        self.prepare_cookies(cookies)
        self.prepare_body(data, files, json)
        self.prepare_auth(auth, url)

    def prepare_method(self, method):
        self.method = method
        if self.method is not None:
            self.method = self.method.upper()

    def prepare_url(self, url, params):
        if isinstance(url, bytes):
            url = url.decode("utf8")
        else:
            url = str(url)
        url = url.lstrip()
        if ":" in url and not url.lower().startswith("http"):
            self.url = url
            return
        try:
            u = parse_url(url)
        except _LocationParseError as e:
            raise InvalidURL(*e.args)
        if not u.scheme:
            raise MissingSchema("Invalid URL " + repr(url) + ": No scheme supplied. Perhaps you meant https://" + url + "?")
        if not u.host:
            raise InvalidURL("Invalid URL " + repr(url) + ": No host supplied")
        host = u.host
        if not host.isascii():
            raise InvalidURL("URL has an invalid label.")
        elif host.startswith(("*", ".")):
            raise InvalidURL("URL has an invalid label.")
        netloc = u.auth or ""
        if netloc:
            netloc += "@"
        netloc += host
        if u.port:
            netloc += ":" + str(u.port)
        path = u.path
        if not path:
            path = "/"
        query = u.query
        if isinstance(params, bytes):
            params = params.decode("utf-8")
        if params is not None:
            enc_params = _encode_params(params)
        else:
            enc_params = ""
        if enc_params:
            if query:
                query = query + "&" + enc_params
            else:
                query = enc_params
        self.url = requote_uri(_urlunparse(u.scheme, netloc, path, query, u.fragment))

    def prepare_headers(self, headers):
        self.headers = CaseInsensitiveDict()
        if headers:
            for name, value in headers.items():
                _check_header(name, value)
                self.headers[name] = value

    def prepare_body(self, data, files, json=None):
        body = None
        content_type = None
        if not data and json is not None:
            content_type = "application/json"
            try:
                body = complexjson.dumps(json, allow_nan=False)
            except ValueError as ve:
                raise InvalidJSONError(ve, request=self)
            if not isinstance(body, bytes):
                body = body.encode("utf-8")
        is_stream = hasattr(data, "__iter__") and not isinstance(data, (str, bytes, list, tuple, dict, CaseInsensitiveDict))
        if is_stream:
            body = data
            self.headers["Transfer-Encoding"] = "chunked"
        else:
            if data:
                body = _encode_params(data)
                if isinstance(data, (str, bytes)):
                    content_type = None
                else:
                    content_type = "application/x-www-form-urlencoded"
            self.prepare_content_length(body)
            if content_type and ("content-type" not in self.headers):
                self.headers["Content-Type"] = content_type
        self.body = body

    def prepare_content_length(self, body):
        if body is not None:
            length = super_len(body)
            if length:
                self.headers["Content-Length"] = str(length)
        elif self.method not in ("GET", "HEAD") and self.headers.get("Content-Length") is None:
            self.headers["Content-Length"] = "0"

    def prepare_auth(self, auth, url=""):
        if auth is None:
            url_auth = get_auth_from_url(self.url)
            auth = url_auth if any(url_auth) else None
        if auth:
            if isinstance(auth, tuple) and len(auth) == 2:
                auth_handler = HTTPBasicAuth(*auth)
            else:
                auth_handler = auth
            r = auth_handler(self)
            if r is not self:
                self.method = r.method
                self.url = r.url
                self.headers = r.headers
                self.body = r.body
            self.prepare_content_length(self.body)

    def prepare_cookies(self, cookies):
        if isinstance(cookies, RequestsCookieJar):
            self._cookies = cookies
        else:
            self._cookies = cookiejar_from_dict(cookies)
        p = urlsplit(self.url)
        h = self._cookies._header_for(p.hostname or "", p.path or "/", p.scheme == "https")
        if h is not None and "Cookie" not in self.headers:
            self.headers["Cookie"] = h

    @property
    def path_url(self):
        p = urlsplit(self.url)
        path = p.path
        if not path:
            path = "/"
        if p.query:
            path = path + "?" + p.query
        return path


def _urlunparse(scheme, netloc, path, query, fragment):
    return urlunsplit(scheme, netloc, path, query or "", fragment or "")


def _check_header(name, value):
    if not isinstance(name, (str, bytes)):
        raise InvalidHeader("Header part (" + repr(name) + ") from " + repr((name, value)) + " must be of type str or bytes, not " + str(type(name)))
    if not isinstance(value, (str, bytes)):
        raise InvalidHeader("Header part (" + repr(value) + ") from " + repr((name, value)) + " must be of type str or bytes, not " + str(type(value)))
    if isinstance(name, str) and (name != name.strip() or ":" in name or "\n" in name or "\r" in name or not name):
        raise InvalidHeader("Invalid leading whitespace, reserved character(s), or return character(s) in header name: " + repr(name))
    if isinstance(value, str) and (value[:1] in (" ", "\t") or "\n" in value or "\r" in value):
        raise InvalidHeader("Invalid leading whitespace, reserved character(s), or return character(s) in header value: " + repr(value))


class Response:
    def __init__(self):
        self._content = False
        self._content_consumed = False
        self._next = None
        self.status_code = None
        self.headers = CaseInsensitiveDict()
        self.raw = None
        self.url = None
        self.encoding = None
        self.history = []
        self.reason = None
        self.cookies = cookiejar_from_dict({})
        self.elapsed = datetime.timedelta(0)
        self.request = None
        self._body = None
        self._conn = None
        self._decoders = None
        self._pool = None
        self._read_err = None

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()

    def __repr__(self):
        return "<Response [" + str(self.status_code) + "]>"

    def __bool__(self):
        return self.ok

    def __iter__(self):
        return self.iter_content(128)

    @property
    def ok(self):
        try:
            self.raise_for_status()
        except HTTPError:
            return False
        return True

    @property
    def is_redirect(self):
        return "location" in self.headers and self.status_code in (301, 302, 303, 307, 308)

    @property
    def is_permanent_redirect(self):
        return "location" in self.headers and self.status_code in (301, 308)

    @property
    def next(self):
        return self._next

    @property
    def apparent_encoding(self):
        return _apparent_encoding(self.content)

    def _raw_chunks(self, want=0):
        # the decoded body as it arrives (a chunked body in pieces of at most want bytes of each HTTP chunk)
        while self._body is not None:
            b = self._next_piece(want)
            if not b:
                self._release()
                return
            yield b

    def _next_piece(self, want=0):
        # the next decoded bytes of the body (b"" at its end); an error releases the connection first
        body = self._body
        try:
            while True:
                b = body.next_sync(want)
                if not b:
                    if self._decoders:
                        return b""
                    return b""
                if self._decoders:
                    for d in self._decoders:
                        b = d.feed(b)
                    if not b:
                        continue
                return b
        except (_brevi_http.Closed, _brevi_http.Protocol):
            self._fail()
            raise ChunkedEncodingError(_protocol_text("Response ended prematurely"))
        except TimeoutError:
            self._fail()
            raise ConnectionError(self._pool_text() + ": Read timed out.")
        except _brevi_http.DecodeFailed as e:
            self._fail()
            raise ContentDecodingError(_decode_text(self.headers.get("content-encoding", ""), e))

    def _fail(self):
        if self._conn is not None:
            self._conn.close()
            self._conn = None
        self._body = None

    def _pool_text(self):
        p = urlsplit(self.url or "")
        port = p.port if p.port is not None else (443 if p.scheme == "https" else 80)
        return ("HTTPSConnectionPool" if p.scheme == "https" else "HTTPConnectionPool") + "(host=" + repr(p.hostname) + ", port=" + str(port) + ")"

    def _release(self):
        body = self._body
        if body is None:
            return
        self._body = None
        if self._decoders:
            for d in self._decoders:
                d.close()
        if self._conn is not None:
            if body.clean and self._conn.reusable and self._pool is not None:
                self._pool._put(self._conn)
            else:
                self._conn.close()
            self._conn = None

    def iter_content(self, chunk_size=1, decode_unicode=False):
        def generate():
            buf = b""
            if self._body is not None and self._body.frame == "chunked":
                # urllib3's read_chunked: each HTTP chunk, in pieces of at most chunk_size
                for b in self._raw_chunks(chunk_size or 0):
                    yield b
                self._content_consumed = True
                return
            for b in self._raw_chunks():
                if chunk_size is None:
                    yield b
                    continue
                buf += b
                while len(buf) >= chunk_size:
                    yield buf[:chunk_size]
                    buf = buf[chunk_size:]
            if buf:
                yield buf
            self._content_consumed = True

        if self._content_consumed and isinstance(self._content, bool):
            raise StreamConsumedError()
        elif chunk_size is not None and not isinstance(chunk_size, int):
            raise TypeError("chunk_size must be an int, it is instead a " + str(type(chunk_size)) + ".")
        if self._content_consumed:
            chunks = iter_slices(self._content, chunk_size)
        else:
            chunks = generate()
        if decode_unicode:
            chunks = stream_decode_response_unicode(chunks, self)
        return chunks

    def iter_lines(self, chunk_size=ITER_CHUNK_SIZE, decode_unicode=False, delimiter=None):
        pending = None
        for chunk in self.iter_content(chunk_size=chunk_size, decode_unicode=decode_unicode):
            if pending is not None:
                chunk = pending + chunk
            if delimiter:
                lines = chunk.split(delimiter)
            else:
                lines = chunk.splitlines()
            if lines and lines[-1] and chunk and lines[-1][-1] == chunk[-1]:
                pending = lines.pop()
            else:
                pending = None
            yield from lines
        if pending is not None:
            yield pending

    @property
    def content(self):
        if self._content is False:
            if self._content_consumed:
                raise RuntimeError("The content for this response was already consumed")
            if self.status_code == 0 or self._body is None and self._conn is None and self.raw is None and self._read_err is None and self.status_code is None:
                self._content = None
            else:
                self._content = b"".join(self.iter_content(CONTENT_CHUNK_SIZE)) or b""
        self._content_consumed = True
        return self._content

    @property
    def text(self):
        encoding = self.encoding
        if not self.content:
            return ""
        if self.encoding is None:
            encoding = self.apparent_encoding
        try:
            return self.content.decode(encoding or "utf-8", "replace")
        except (LookupError, TypeError):
            return self.content.decode("utf-8", "replace")

    def json(self, **kwargs):
        if not self.encoding and self.content and len(self.content) > 3:
            encoding = guess_json_utf(self.content)
            if encoding is not None:
                try:
                    return complexjson.loads(self.content.decode(encoding), **kwargs)
                except UnicodeDecodeError:
                    pass
                except complexjson.JSONDecodeError as e:
                    raise JSONDecodeError(e.msg, e.doc, e.pos)
        try:
            return complexjson.loads(self.text, **kwargs)
        except complexjson.JSONDecodeError as e:
            raise JSONDecodeError(e.msg, e.doc, e.pos)

    @property
    def links(self):
        header = self.headers.get("link")
        resolved_links = {}
        if header:
            for link in parse_header_links(header):
                key = link.get("rel") or link.get("url")
                if key is not None:
                    resolved_links[key] = link
        return resolved_links

    def raise_for_status(self):
        http_error_msg = ""
        reason = self.reason
        if isinstance(reason, bytes):
            try:
                reason = reason.decode("utf-8")
            except UnicodeDecodeError:
                reason = reason.decode("iso-8859-1")
        if 400 <= self.status_code < 500:
            http_error_msg = str(self.status_code) + " Client Error: " + str(reason) + " for url: " + str(self.url)
        elif 500 <= self.status_code < 600:
            http_error_msg = str(self.status_code) + " Server Error: " + str(reason) + " for url: " + str(self.url)
        if http_error_msg:
            raise HTTPError(http_error_msg, response=self)

    def close(self):
        if not self._content_consumed:
            if self._conn is not None:
                self._conn.close()
                self._conn = None
            self._body = None
        else:
            self._release()


def _protocol_text(msg):
    return "('" + msg + "', IncompleteRead)"


def _decode_text(enc, e):
    return "('Received response with content-encoding: " + enc + ", but failed to decode it.', error(" + repr(str(e)) + "))"


# ---------- sending (urllib3's pool, cut down) ----------

_REASONS = {
    100: "Continue", 101: "Switching Protocols", 200: "OK", 201: "Created", 202: "Accepted", 204: "No Content",
    301: "Moved Permanently", 302: "Found", 303: "See Other", 304: "Not Modified", 307: "Temporary Redirect",
    308: "Permanent Redirect", 400: "Bad Request", 401: "Unauthorized", 403: "Forbidden", 404: "Not Found",
    405: "Method Not Allowed", 409: "Conflict", 418: "I'm a Teapot", 422: "Unprocessable Entity",
    429: "Too Many Requests", 500: "Internal Server Error", 502: "Bad Gateway", 503: "Service Unavailable",
    504: "Gateway Timeout",
}


class _Pool:
    def __init__(self):
        self.idle = {}

    def _get(self, key):
        lst = self.idle.get(key)
        if lst:
            return lst.pop()
        return None

    def _put(self, conn):
        self.idle.setdefault(conn.key, []).append(conn)

    def close(self):
        for lst in self.idle.values():
            for c in lst:
                c.close()
        self.idle = {}


def _timeouts(timeout):
    if isinstance(timeout, tuple):
        if len(timeout) != 2:
            raise ValueError("Invalid timeout " + repr(timeout) + ". Pass a (connect, read) timeout tuple, or a single float to set both timeouts to the same value.")
        return timeout[0], timeout[1]
    return timeout, timeout


def _body_bytes(body):
    if body is None:
        return b""
    if isinstance(body, str):
        return body.encode("utf-8")
    return body


def _send(pool, request, stream=False, timeout=None, verify=True, proxies=None):
    p = urlsplit(request.url)
    scheme = p.scheme
    host = p.hostname or ""
    port = p.port if p.port is not None else (443 if scheme == "https" else 80)
    connect_t, read_t = _timeouts(timeout)
    pool_name = ("HTTPSConnectionPool" if scheme == "https" else "HTTPConnectionPool") + "(host=" + repr(host) + ", port=" + str(port) + ")"
    conn_name = ("HTTPSConnection" if scheme == "https" else "HTTPConnection") + "(host=" + repr(host) + ", port=" + str(port) + ")"
    url_text = request.path_url
    proxy = select_proxy(request.url, proxies)
    proxy_t = None
    target = request.path_url
    headers = []
    for k, v in request.headers.items():
        if isinstance(k, bytes):
            k = k.decode("latin-1")
        if isinstance(v, bytes):
            v = v.decode("latin-1")
        headers.append((k, v))
    if proxy:
        pp = parse_url(proxy if "://" in proxy else "http://" + proxy)
        pauth = None
        if pp.auth:
            u, _, pw = pp.auth.partition(":")
            pauth = _basic_auth_str(unquote(u), unquote(pw))
        proxy_t = (pp.host, pp.port or (443 if pp.scheme == "https" else 80), pauth)
        if scheme == "http":
            target = request.url.split("#")[0]
            if pauth and not any(k.lower() == "proxy-authorization" for k, v in headers):
                headers.append(("Proxy-Authorization", pauth))
    # Host goes first, as http.client puts it (unless the program set one)
    if not any(k.lower() == "host" for k, v in headers):
        headers.insert(0, ("Host", _brevi_http.host_header(host, port, scheme)))
    if isinstance(verify, str):
        # a CA bundle file, or a directory of certificates (the native side tells which)
        tls = (True, verify, "", True)
    else:
        tls = (bool(verify), "", "", True)
    key = (scheme, host, port, tls, proxy_t)
    body = request.body
    chunked = body is not None and not isinstance(body, (str, bytes))
    data = _brevi_http.request_bytes(request.method, target, headers)
    for attempt in range(2):
        conn = pool._get(key)
        fresh = conn is None
        if fresh:
            try:
                cid = _brevi_http.connect(scheme, host, port, connect_t, tls, proxy_t)
            except BaseException as e:
                _raise_connect(e, pool_name, conn_name, host, url_text, connect_t, scheme, request, proxy_t)
            conn = _brevi_http.Conn(cid, key)
        try:
            _brevi_net_settimeout(conn.id, read_t)
            if chunked:
                _brevi_http.send_sync(conn.id, data)
                for piece in body:
                    if isinstance(piece, str):
                        piece = piece.encode("utf-8")
                    if piece:
                        _brevi_http.send_sync(conn.id, ("%x\r\n" % len(piece)).encode() + piece + b"\r\n")
                _brevi_http.send_sync(conn.id, b"0\r\n\r\n")
            else:
                _brevi_http.send_sync(conn.id, data + _body_bytes(body))
            status, reason, version, raw_headers = _brevi_http.read_head(conn.id)
            break
        except TimeoutError:
            conn.close()
            raise ReadTimeout(pool_name + ": Read timed out. (read timeout=" + str(read_t) + ")", request=request)
        except (_brevi_http.Closed, ConnectionResetError, BrokenPipeError) as e:
            conn.close()
            if not fresh:
                continue
            msg = "Remote end closed connection without response" if isinstance(e, _brevi_http.Closed) else str(e)
            inner = "RemoteDisconnected(" + repr(msg) + ")" if isinstance(e, _brevi_http.Closed) else type(e).__name__ + "(" + repr(str(e)) + ")"
            raise ConnectionError("('Connection aborted.', " + inner + ")", request=request)
        except _brevi_http.Protocol as e:
            conn.close()
            raise ConnectionError("('Connection aborted.', BadStatusLine(" + repr(str(e)) + "))", request=request)
        except OSError as e:
            conn.close()
            if type(e).__name__ in ("SSLError", "SSLCertVerificationError"):
                raise SSLError(pool_name + ": Max retries exceeded with url: " + url_text + " (Caused by SSLError(" + repr(str(e)) + "))", request=request)
            raise ConnectionError("('Connection aborted.', " + type(e).__name__ + "(" + repr(str(e)) + "))", request=request)
    resp = Response()
    resp.status_code = status
    hdrs = CaseInsensitiveDict()
    text_headers = []
    for k, v in raw_headers:
        ks = k.decode("latin-1")
        vs = v.decode("latin-1")
        text_headers.append((ks, vs))
        if ks in hdrs:
            hdrs[ks] = hdrs[ks] + ", " + vs
        else:
            hdrs[ks] = vs
    resp.headers = hdrs
    resp.encoding = get_encoding_from_headers(hdrs)
    resp.reason = reason
    resp.url = request.url
    resp.request = request
    resp._text_headers = text_headers
    host_c = host
    resp.cookies._extract(host_c, urlsplit(request.url).path or "/", text_headers)
    frame = _brevi_http.framing(request.method, status, raw_headers)
    resp._body = _brevi_http.Body(conn.id, frame)
    resp._conn = conn
    resp._pool = pool
    conn.reusable = frame[0] != "close" and hdrs.get("connection", "").lower() != "close" and version != "HTTP/1.0"
    enc = hdrs.get("content-encoding")
    if enc and request.method != "HEAD":
        resp._decoders = _brevi_http.decoders(enc)
    if resp._body.done:
        resp._release()
    return resp


def _brevi_net_settimeout(cid, t):
    import _brevi_net
    _brevi_net.settimeout(cid, -1.0 if t is None else float(t))


def _raise_connect(e, pool_name, conn_name, host, url_text, connect_t, scheme, request, proxy_t):
    name = type(e).__name__
    if name == "PermissionError":
        raise ConnectionError(str(e), request=request)
    if proxy_t is not None and name not in ("SSLError", "SSLCertVerificationError"):
        raise ProxyError(pool_name + ": Max retries exceeded with url: " + url_text + " (Caused by ProxyError('Unable to connect to proxy', " + name + "(" + repr(str(e)) + ")))", request=request)
    if name == "gaierror":
        inner = conn_name + ": Failed to resolve '" + host + "' (" + str(e) + ")"
        raise ConnectionError(pool_name + ": Max retries exceeded with url: " + url_text + " (Caused by NameResolutionError(" + repr(inner) + "))", request=request)
    if name == "TimeoutError":
        inner = "ConnectTimeoutError(<" + conn_name + " at 0x7f0000000000>, " + repr("Connection to " + host + " timed out. (connect timeout=" + str(connect_t) + ")") + ")"
        raise ConnectTimeout(pool_name + ": Max retries exceeded with url: " + url_text + " (Caused by " + inner + ")", request=request)
    if name in ("SSLError", "SSLCertVerificationError"):
        inner = name + "(1, " + repr(str(e)) + ")" if name == "SSLCertVerificationError" else name + "(" + repr(str(e)) + ")"
        raise SSLError(pool_name + ": Max retries exceeded with url: " + url_text + " (Caused by SSLError(" + inner + "))", request=request)
    if isinstance(e, OSError):
        inner = conn_name + ": Failed to establish a new connection: " + str(e)
        raise ConnectionError(pool_name + ": Max retries exceeded with url: " + url_text + " (Caused by NewConnectionError(" + repr(inner) + "))", request=request)
    raise e


# ---------- sessions ----------

DEFAULT_PORTS = {"http": 80, "https": 443}


class Session:
    def __init__(self):
        self.headers = default_headers()
        self.auth = None
        self.proxies = {}
        self.hooks = {"response": []}
        self.params = {}
        self.stream = False
        self.verify = True
        self.cert = None
        self.max_redirects = DEFAULT_REDIRECT_LIMIT
        self.trust_env = True
        self.cookies = cookiejar_from_dict({})
        self._pool = _Pool()

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()

    def __repr__(self):
        return "<requests.sessions.Session object at 0x7f0000000000>"

    def close(self):
        self._pool.close()

    def prepare_request(self, request):
        cookies = request.cookies or {}
        if not isinstance(cookies, RequestsCookieJar):
            cookies = cookiejar_from_dict(cookies)
        merged = RequestsCookieJar()
        merged.update(self.cookies)
        merged.update(cookies)
        p = PreparedRequest()
        p.prepare(
            method=request.method.upper(),
            url=request.url,
            files=request.files,
            data=request.data,
            json=request.json,
            headers=merge_setting(request.headers, self.headers, dict_class=CaseInsensitiveDict),
            params=merge_setting(request.params, self.params),
            auth=merge_setting(request.auth, self.auth),
            cookies=merged,
        )
        return p

    def request(self, method, url, params=None, data=None, headers=None, cookies=None, files=None, auth=None, timeout=None, allow_redirects=True, proxies=None, hooks=None, stream=None, verify=None, cert=None, json=None):
        if isinstance(url, bytes):
            url = url.decode("utf-8")
        req = Request(method=method.upper(), url=url, headers=headers, files=files, data=data or {}, json=json, params=params or {}, auth=auth, cookies=cookies)
        prep = self.prepare_request(req)
        proxies = proxies or {}
        settings = self.merge_environment_settings(prep.url, proxies, stream, verify, cert)
        return self.send(prep, timeout=timeout, allow_redirects=allow_redirects, **settings)

    def merge_environment_settings(self, url, proxies, stream, verify, cert):
        if self.trust_env:
            if verify is True or verify is None:
                verify = _brevi_http.env_exact("REQUESTS_CA_BUNDLE") or _brevi_http.env_exact("CURL_CA_BUNDLE") or verify
        proxies = merge_setting(proxies, self.proxies)
        stream = merge_setting(stream, self.stream)
        verify = merge_setting(verify, self.verify)
        cert = merge_setting(cert, self.cert)
        return {"proxies": proxies, "stream": stream, "verify": verify, "cert": cert}

    def get(self, url, params=None, **kwargs):
        kwargs.setdefault("allow_redirects", True)
        return self.request("GET", url, params=params, **kwargs)

    def options(self, url, **kwargs):
        kwargs.setdefault("allow_redirects", True)
        return self.request("OPTIONS", url, **kwargs)

    def head(self, url, **kwargs):
        kwargs.setdefault("allow_redirects", False)
        return self.request("HEAD", url, **kwargs)

    def post(self, url, data=None, json=None, **kwargs):
        return self.request("POST", url, data=data, json=json, **kwargs)

    def put(self, url, data=None, **kwargs):
        return self.request("PUT", url, data=data, **kwargs)

    def patch(self, url, data=None, **kwargs):
        return self.request("PATCH", url, data=data, **kwargs)

    def delete(self, url, **kwargs):
        return self.request("DELETE", url, **kwargs)

    def _adapter_check(self, url):
        if not url.lower().startswith(("https://", "http://")):
            raise InvalidSchema("No connection adapters were found for " + repr(url))

    def send(self, request, **kwargs):
        kwargs.setdefault("stream", self.stream)
        kwargs.setdefault("verify", self.verify)
        kwargs.setdefault("cert", self.cert)
        if "proxies" not in kwargs or kwargs["proxies"] is None:
            kwargs["proxies"] = {}
        kwargs["proxies"] = resolve_proxies(request.url, kwargs["proxies"] or self.proxies, self.trust_env)
        allow_redirects = kwargs.pop("allow_redirects", True)
        stream = kwargs.get("stream")
        self._adapter_check(request.url)
        start = time.perf_counter()
        r = _send(self._pool, request, stream=stream, timeout=kwargs.get("timeout"), verify=kwargs.get("verify"), proxies=kwargs.get("proxies"))
        r.elapsed = datetime.timedelta(seconds=time.perf_counter() - start)
        self.cookies.update(r.cookies)
        if allow_redirects:
            history = list(self._redirects(r, request, **kwargs))
        else:
            history = []
        if history:
            history.insert(0, r)
            r = history.pop()
            r.history = history
        if not allow_redirects:
            if r.is_redirect:
                nxt = self._next_request(r, request)
                r._next = nxt
        if not stream:
            r.content
        return r

    def get_redirect_target(self, resp):
        if resp.is_redirect:
            location = resp.headers["location"]
            location = location.encode("latin1")
            return location.decode("utf8", "replace")
        return None

    def should_strip_auth(self, old_url, new_url):
        old_parsed = urlsplit(old_url)
        new_parsed = urlsplit(new_url)
        if old_parsed.hostname != new_parsed.hostname:
            return True
        if old_parsed.scheme == "http" and old_parsed.port in (80, None) and new_parsed.scheme == "https" and new_parsed.port in (443, None):
            return False
        changed_port = old_parsed.port != new_parsed.port
        changed_scheme = old_parsed.scheme != new_parsed.scheme
        default_port = (DEFAULT_PORTS.get(old_parsed.scheme, None), None)
        if not changed_scheme and old_parsed.port in default_port and new_parsed.port in default_port:
            return False
        return changed_port or changed_scheme

    def _next_request(self, resp, req, previous_fragment=None):
        url = self.get_redirect_target(resp)
        if previous_fragment is None:
            previous_fragment = urlsplit(req.url).fragment
        prepared_request = req.copy()
        if url.startswith("//"):
            url = urlsplit(resp.url).scheme + ":" + url
        parsed = urlsplit(url)
        if parsed.fragment == "" and previous_fragment:
            parsed.fragment = previous_fragment
        url = parsed.geturl()
        if not parsed.netloc:
            url = urljoin(resp.url, requote_uri(url))
        else:
            url = requote_uri(url)
        prepared_request.url = url
        method = prepared_request.method
        if resp.status_code == 303 and method != "HEAD":
            method = "GET"
        if resp.status_code == 302 and method != "HEAD":
            method = "GET"
        if resp.status_code == 301 and method == "POST":
            method = "GET"
        prepared_request.method = method
        if resp.status_code not in (307, 308):
            for header in ("Content-Length", "Content-Type", "Transfer-Encoding"):
                prepared_request.headers.pop(header, None)
            prepared_request.body = None
        headers = prepared_request.headers
        headers.pop("Cookie", None)
        jar = prepared_request._cookies
        jar.update(resp.cookies)
        jar.update(self.cookies)
        prepared_request._cookies = None
        prepared_request.prepare_cookies(jar)
        if "Authorization" in headers and self.should_strip_auth(resp.request.url, prepared_request.url):
            del headers["Authorization"]
        return prepared_request

    def _redirects(self, resp, req, stream=False, timeout=None, verify=True, cert=None, proxies=None):
        hist = []
        url = self.get_redirect_target(resp)
        previous_fragment = urlsplit(req.url).fragment
        while url:
            resp.history = hist[:]
            hist.append(resp)
            try:
                resp.content
            except (ChunkedEncodingError, ContentDecodingError, RuntimeError):
                pass
            if len(resp.history) >= self.max_redirects:
                raise TooManyRedirects("Exceeded " + str(self.max_redirects) + " redirects.", response=resp)
            resp.close()
            prepared_request = self._next_request(resp, req, previous_fragment)
            f = urlsplit(prepared_request.url).fragment
            if f:
                previous_fragment = f
            self._adapter_check(prepared_request.url)
            px = resolve_proxies(prepared_request.url, proxies, self.trust_env)
            req = prepared_request
            start = time.perf_counter()
            resp = _send(self._pool, req, stream=stream, timeout=timeout, verify=verify, proxies=px)
            resp.elapsed = datetime.timedelta(seconds=time.perf_counter() - start)
            self.cookies.update(resp.cookies)
            url = self.get_redirect_target(resp)
            yield resp


def session():
    return Session()


# ---------- the module functions ----------

def request(method, url, **kwargs):
    with Session() as s:
        return s.request(method=method, url=url, **kwargs)


def get(url, params=None, **kwargs):
    return request("get", url, params=params, **kwargs)


def options(url, **kwargs):
    return request("options", url, **kwargs)


def head(url, **kwargs):
    kwargs.setdefault("allow_redirects", False)
    return request("head", url, **kwargs)


def post(url, data=None, json=None, **kwargs):
    return request("post", url, data=data, json=json, **kwargs)


def put(url, data=None, **kwargs):
    return request("put", url, data=data, **kwargs)


def patch(url, data=None, **kwargs):
    return request("patch", url, data=data, **kwargs)


def delete(url, **kwargs):
    return request("delete", url, **kwargs)
