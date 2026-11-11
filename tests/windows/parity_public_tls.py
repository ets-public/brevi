# HTTPS to a public site: the certificate must verify against the public roots Brevi trusts on Windows,
# with no CA file given. The mixed-type value keeps the program on Brevi's own requests (dynamic path).
import requests

r = requests.get("https://www.python.org/", timeout=30)
print(r.status_code, r.url.startswith("https://www.python.org/"))
v = 1
v = "one"
print(v)
