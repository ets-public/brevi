# requests: sessions, redirects, cookies, auth, timeouts, errors, streaming and HTTPS
import json
import requests
from requests.exceptions import HTTPError, ConnectionError, Timeout
from requests.auth import HTTPBasicAuth

B = "http://127.0.0.1:18080"
S = "https://127.0.0.1:18081"
CERT = "tests/http/cert.pem"

with requests.Session() as s:
    s.headers.update({"X-Session": "yes", "Accept": "application/json"})
    s.params = {"sp": "1"}
    r = s.get(B + "/echo", params={"rp": "2"}, headers={"X-Req": "r", "Accept": None})
    print(r.url, r.json()["headers"])
    r = s.get(B + "/cookies/set?flavor=oat&n=2")
    print(r.status_code, r.url, r.json(), [h.status_code for h in r.history])
    print(sorted(s.cookies.get_dict().items()), s.cookies.get("flavor"), "n" in s.cookies)
    r = s.get(B + "/cookies", cookies={"extra": "x"})
    print(r.json())
    r = s.get(B + "/basic-auth/ann/pw", auth=("ann", "pw"))
    print(r.status_code, r.json())
    r = s.get(B + "/basic-auth/ann/pw", auth=HTTPBasicAuth("ann", "bad"))
    print(r.status_code, r.ok, r.headers.get("www-authenticate"))
    s.auth = ("ann", "pw")
    print(s.get(B + "/basic-auth/ann/pw").json())

# redirects
r = requests.get(B + "/redirect/3")
print(r.status_code, r.url, [(h.status_code, h.url) for h in r.history], r.json()["method"])
r = requests.get(B + "/redirect/2", allow_redirects=False)
print(r.status_code, r.is_redirect, r.headers["location"], r.next.url, r.next.method)
for code in [301, 302, 303, 307, 308]:
    r = requests.post(B + "/redirect-to", params={"url": "/echo", "status": code}, data="payload")
    print(code, r.json()["method"], r.json()["body"], [h.status_code for h in r.history])
r = requests.get(B + "/absolute-redirect")
print(r.url)
r = requests.get(B + "/redirect-to?url=/echo%3Fq%3D1%23frag")
print(r.url)
s = requests.Session()
s.max_redirects = 2
try:
    s.get(B + "/redirect/5")
except requests.TooManyRedirects as e:
    print("TooManyRedirects", e, e.response.status_code)
s.close()

# errors
r = requests.get(B + "/status/404")
try:
    r.raise_for_status()
except HTTPError as e:
    print("HTTPError", e, e.response.status_code, e.request.url, isinstance(e, requests.RequestException), isinstance(e, OSError))
for code in [500, 503]:
    try:
        requests.post(B + "/status/%d" % code).raise_for_status()
    except requests.exceptions.HTTPError as e:
        print(e)
try:
    requests.get("http://127.0.0.1:1/x")
except ConnectionError as e:
    print("ConnectionError", e)
try:
    requests.get(B + "/slow?d=1", timeout=0.2)
except Timeout as e:
    print(type(e).__name__, e, isinstance(e, requests.ReadTimeout))
try:
    requests.get(B + "/slow?d=1", timeout=(2, 0.2))
except requests.ReadTimeout as e:
    print("ReadTimeout", e)
print(requests.get(B + "/slow?d=0.1", timeout=5).json())
for bad in ["ftp://x/y", "localhost:80", "http://", "nohost", "http://*bad/"]:
    try:
        requests.get(bad)
    except requests.RequestException as e:
        print(type(e).__name__, e)
try:
    requests.get(B + "/bad-json").json()
except requests.JSONDecodeError as e:
    print("JSONDecodeError", e, e.pos, isinstance(e, ValueError), isinstance(e, json.JSONDecodeError))
try:
    requests.get(B + "/echo", headers={"X-Bad": "a\nb"})
except requests.exceptions.InvalidHeader as e:
    print("InvalidHeader", e)

# streaming
r = requests.get(B + "/chunked", stream=True)
print([c for c in r.iter_content(chunk_size=8)][:4])
r = requests.get(B + "/chunked", stream=True)
print(list(r.iter_lines(decode_unicode=True)))
r = requests.get(B + "/sse?n=4", stream=True)
for line in r.iter_lines():
    if line.startswith(b"data: "):
        payload = line[6:]
        if payload == b"[DONE]":
            print("done")
            break
        print(json.loads(payload))
r = requests.get(B + "/utf8", stream=True)
print(list(r.iter_content(chunk_size=3, decode_unicode=True)))
r = requests.get(B + "/gzip", stream=True)
print(b"".join(r.iter_content(chunk_size=5)))
with requests.get(B + "/big", stream=True) as r:
    n = 0
    for c in r.iter_content(chunk_size=65536):
        n += len(c)
    print(n)
r = requests.get(B + "/utf8", stream=True)
print(r.content, r.text)
try:
    list(r.iter_content())
    print("again ok")
except requests.exceptions.StreamConsumedError as e:
    print("consumed")

# HTTPS
r = requests.get(S + "/echo?tls=1", verify=CERT)
print(r.status_code, r.url, r.json()["query"])
r = requests.get(S + "/gzip", verify=CERT)
print(r.text.count("gzipped"))
try:
    requests.get(S + "/echo")
except requests.exceptions.SSLError as e:
    print("SSLError", e, isinstance(e, ConnectionError))
# (verify given with each request: a session's verify gives way to $REQUESTS_CA_BUNDLE)
s = requests.Session()
print(s.get("https://localhost:18081/echo", verify=CERT).json()["headers"][0])
print(s.get(S + "/chunked", verify=CERT).text.splitlines())
print(s.get(S + "/echo", verify=False).status_code)
