# httpx: AsyncClient on asyncio, streaming (sync and async), request bodies from generators, auth flows,
# event hooks, the errors a program sees, and the URL / QueryParams / Headers / Response objects on their own
import asyncio
import httpx

B = "http://127.0.0.1:18080"
S = "https://127.0.0.1:18081"
CERT = "tests/http/cert.pem"


def show(f):
    try:
        r = f()
        print("ok", r)
    except httpx.HTTPStatusError as e:
        print(type(e).__name__, str(e).splitlines()[0], e.response.status_code, e.request.url)
    except httpx.HTTPError as e:
        print(type(e).__name__, repr(str(e)), e.request.url, isinstance(e, httpx.TransportError))
    except Exception as e:
        print(type(e).__name__, repr(str(e)))


# streaming, sync
with httpx.stream("GET", B + "/chunked") as r:
    print(r.status_code, r.is_stream_consumed, r.is_closed, [line for line in r.iter_lines()], r.is_closed)
with httpx.Client() as c:
    with c.stream("GET", B + "/big") as r:
        sizes = [len(b) for b in r.iter_bytes(chunk_size=100000)]
        print(sizes, r.num_bytes_downloaded)
    with c.stream("GET", B + "/big?gz=1") as r:
        total = 0
        for b in r.iter_bytes():
            total += len(b)
        print(total, r.num_bytes_downloaded < total)
    with c.stream("GET", B + "/sse?n=4&gap=0.01") as r:
        events = [l for l in r.iter_lines() if l.startswith("data: ")]
        print(events)
    with c.stream("GET", B + "/utf8") as r:
        print([t for t in r.iter_text(chunk_size=4)])
    with c.stream("GET", B + "/echo") as r:
        try:
            r.content
        except httpx.ResponseNotRead as e:
            print("ResponseNotRead", e)
        r.read()
        print(len(r.content))
        try:
            list(r.iter_lines())
            print("re-iterated from content")
        except httpx.StreamConsumed as e:
            print("StreamConsumed")

    def gen():
        yield b"part one, "
        yield b"part two"
        yield b""
        yield b", three"

    r = c.post(B + "/echo", content=gen())
    print(r.json()["body"], r.json()["headers"][-1])

    class Bearer(httpx.Auth):
        def __init__(self, token):
            self.token = token

        def auth_flow(self, request):
            request.headers["Authorization"] = "Bearer " + self.token
            yield request

    r = c.get(B + "/echo", auth=Bearer("tok123"))
    print(r.json()["headers"][-1], r.request.headers)

    seen = []
    c.event_hooks = {"request": [lambda req: seen.append(("req", req.method, str(req.url)))], "response": [lambda resp: seen.append(("resp", resp.status_code))]}
    c.get(B + "/redirect/2", follow_redirects=True)
    print(seen)
    c.event_hooks = {}
    r = c.get(B + "/echo", auth=lambda req: req)
    print(r.status_code, c.timeout, c.cookies)

# errors, sync
show(lambda: httpx.get("http://127.0.0.1:1/"))
show(lambda: httpx.get(B + "/slow?d=1", timeout=0.2))
show(lambda: httpx.get(S + "/echo", verify=True))
show(lambda: httpx.get("ftp://example.com/"))
show(lambda: httpx.get("/relative"))
show(lambda: httpx.get(B + "/status/404").raise_for_status())
show(lambda: httpx.get(B + "/status/503").raise_for_status())
show(lambda: httpx.get(B + "/redirect/1").raise_for_status())
show(lambda: httpx.get(B + "/echo").raise_for_status())
show(lambda: httpx.get(B + "/bad-json").json())
show(lambda: httpx.URL("http://[::1"))
show(lambda: httpx.URL("http://300.1.1.1/"))
show(lambda: httpx.URL("http://x:port/"))
show(lambda: httpx.Timeout())
show(lambda: httpx.Client().get(B + "/echo", timeout=httpx.Timeout(2.0, connect=1.0)).status_code)
c = httpx.Client()
c.close()
show(lambda: c.get(B + "/echo"))


# async
async def fetch(client, i):
    r = await client.get("/echo", params={"i": i})
    return r.status_code, r.json()["query"]


async def main():
    async with httpx.AsyncClient(base_url=B, headers={"X-A": "1"}) as client:
        results = await asyncio.gather(*[fetch(client, i) for i in range(6)])
        print(results)
        r = await client.post("/echo", json={"a": [1, 2]})
        print(r, r.json()["body"], r.json()["headers"][-3:])
        r = await client.get("/gzip")
        print(r.headers["content-encoding"], r.text.count("gzipped"))
        r = await client.get("/redirect/3", follow_redirects=True)
        print(r.url, len(r.history))
        r = await client.get("/chunked")
        print(repr(r.text))
        async with client.stream("GET", "/sse?n=5&gap=0.02") as resp:
            got = []
            async for line in resp.aiter_lines():
                if line:
                    got.append(line)
            print(resp.status_code, got)
        async with client.stream("GET", "/big") as resp:
            n = 0
            async for b in resp.aiter_bytes(chunk_size=65536):
                n += 1
            print(n, resp.num_bytes_downloaded)
        async with client.stream("GET", "/utf8") as resp:
            print([t async for t in resp.aiter_text()])
        r = await client.get("/basic-auth/u/p", auth=("u", "p"))
        print(r.json())

        async def agen():
            for p in [b"async ", b"body"]:
                await asyncio.sleep(0)
                yield p

        r = await client.put("/echo", content=agen())
        print(r.json()["method"], r.json()["body"])
    print(client.is_closed)


async def errors():
    async with httpx.AsyncClient(verify=CERT) as client:
        r = await client.get(S + "/gzip")
        print(r.status_code, r.text.count("gzipped"))
        for url, kw in [("http://127.0.0.1:1/", {}), (B + "/slow?d=1", {"timeout": 0.2}), ("ftp://x/", {})]:
            try:
                await client.get(url, **kw)
            except httpx.HTTPError as e:
                print("async", type(e).__name__, repr(str(e)))
        # the slow request in the background while others finish
        t0 = asyncio.get_running_loop().time()
        slow = asyncio.create_task(client.get(B + "/slow?d=0.3"))
        quick = await asyncio.gather(*[client.get(B + "/echo?q=" + str(i)) for i in range(3)])
        print([q.status_code for q in quick], slow.done())
        r = await slow
        print(r.json(), asyncio.get_running_loop().time() - t0 < 2)
    async with httpx.AsyncClient() as client:
        try:
            await client.get(S + "/echo")
        except httpx.ConnectError as e:
            print("async ConnectError", str(e)[:40])


asyncio.run(main())
asyncio.run(errors())


# the objects on their own
u = httpx.URL("HTTPS://user:pass@EXAMPLE.com:443/a/./b/../c%2Fd?x=1&y=two words#frag")
print(u, repr(u), u.scheme, u.host, u.port, u.path, u.raw_path, u.query, u.fragment, u.username, u.password, u.userinfo, u.netloc)
print(u.copy_with(scheme="http", port=8080), u.copy_set_param("x", 5), u.copy_add_param("z", True), u.copy_remove_param("x"), u.copy_merge_params({"m": [1, 2]}))
print(u.join("../other?q=1"), u.join("//cdn.example.com/x"), u.join("rel"), u == "https://user:pass@example.com/a/c%2Fd?x=1&y=two%20words#frag", u.is_absolute_url, httpx.URL("/p").is_relative_url)
print(httpx.URL("http://例え.jp/パス?q=値"), httpx.URL("http://xn--r8jz45g.jp/").host, httpx.URL("http://[::1]:8000/").host, httpx.URL("http://[::1]:8000/"))
print(httpx.URL("http://example.com", params={"a": "1", "b": ["2", "3"]}), httpx.URL(httpx.URL("http://h/p"), path="/q"))
q = httpx.QueryParams("a=1&a=2&b=x%20y&c=")
print(q, repr(q), q["a"], q.get_list("a"), list(q.keys()), list(q.items()), q.multi_items(), "b" in q, len(q), q.set("a", "9"), q.add("d", None), q.remove("a"), q.merge({"b": "new"}), q == httpx.QueryParams([("a", "1"), ("a", "2"), ("b", "x y"), ("c", "")]))
q2 = httpx.QueryParams({"flag": False, "n": 1.5, "list": (1, 2)})
print(q2, dict(q2), bool(httpx.QueryParams()))
h = httpx.Headers({"Content-Type": "text/plain", "X-A": "1"})
h["x-b"] = "2"
h.update({"X-A": "3"})
print(h, h.raw, list(h.items()), h.get("missing", "d"), "content-type" in h, h.copy() == h)
del h["x-b"]
h2 = httpx.Headers([("Set-Cookie", "a=1"), ("set-cookie", "b=2"), ("Authorization", "secret")])
print(h2, h2["set-cookie"], h2.get_list("SET-COOKIE"), h2.multi_items(), h == {"content-type": "text/plain", "x-a": "3"})
print(httpx.Headers({"X-Utf": "é".encode("utf-8")}), httpx.Headers({"X-L": b"\xe9"}).encoding)
r = httpx.Response(201, json={"ok": True}, headers={"X-H": "v"})
print(r, r.status_code, r.headers, r.json(), r.text, r.is_success, r.encoding, r.is_closed, r.is_stream_consumed)
r = httpx.Response(200, text="héllo")
print(r.headers, r.content, r.charset_encoding, r.encoding)
r = httpx.Response(200, content=b"\xe9t\xe9", headers={"Content-Type": "text/plain; charset=latin-1"})
print(r.text, r.encoding, list(r.iter_bytes(chunk_size=2)), list(r.iter_text()), list(r.iter_lines()))
try:
    r.request
except RuntimeError as e:
    print("RuntimeError", e)
req = httpx.Request("post", "http://example.com/x?a=1", params={"b": 2}, json={"k": "v"}, headers={"X-R": "1"}, cookies={"c": "d"})
print(req, req.method, req.url, req.headers, req.content, req.read())
t = httpx.Timeout(10.0, connect=2.0)
print(t, httpx.Timeout(3), httpx.Timeout(None), httpx.Timeout((1, 2, 3, 4)), t.as_dict(), t == httpx.Timeout(10.0, connect=2.0), httpx.Limits(max_connections=5))
print(httpx.codes.NOT_FOUND, httpx.codes.NOT_FOUND.phrase, repr(httpx.codes.FOUND), httpx.codes(404).name, httpx.codes.is_error(500), httpx.codes.get_reason_phrase(299), httpx.codes.OK == 200)
ck = httpx.Cookies({"a": "1"})
ck.set("b", "2", domain="example.com")
print(ck, dict(ck), ck.get("b"), len(ck), "a" in ck)
ck.delete("a")
print(ck, list(ck))
print(httpx.BasicAuth("u", "p")._auth_header, issubclass(httpx.ConnectTimeout, httpx.TimeoutException), issubclass(httpx.ReadError, httpx.NetworkError), issubclass(httpx.HTTPStatusError, httpx.RequestError), httpx.__version__)
