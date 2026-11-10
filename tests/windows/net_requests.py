# Brevi's own requests over its native sockets and TLS on Windows: the HTTP and HTTPS sides of
# tests/windows/net_server.py. The mixed-type value at the end keeps the program on the dynamic native path,
# where Brevi's own requests runs: the typed path would hand requests to CPython through libpython instead.
import sys

import requests

port = int(sys.argv[1]) if len(sys.argv) > 1 else 18180
ca = sys.argv[2] if len(sys.argv) > 2 else "tests/http/cert.pem"
B = "http://127.0.0.1:" + str(port)
H = "https://127.0.0.1:" + str(port + 1)

r = requests.get(B + "/echo?a=1")
print(r.status_code, r.ok, r.reason)
print(r.json()["path"], r.json()["query"], r.json()["method"])
print(r.headers["content-type"], r.headers["server"])
print(r.text.strip())

r = requests.get(B + "/json")
print(r.json())
r = requests.get(B + "/gzip")
print(r.headers["content-encoding"], r.json())
r = requests.get(B + "/missing")
print(r.status_code, r.json()["error"])

r = requests.get(H + "/text", verify=ca)
print(r.status_code, repr(r.text))
r = requests.get(H + "/json", verify=ca)
print(r.json())

v = 1
v = "one"
print(v)
