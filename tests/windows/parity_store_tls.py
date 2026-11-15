# HTTPS with no CA file given, against the URL in argv[1]: Brevi's own requests must trust exactly what
# CPython's ssl.create_default_context() trusts on Windows (parity_store_ref.py), which loads the Windows
# certificate store ("CA" and "ROOT", certificates trusted for server authentication). Prints "ok" when the
# certificate verified, "verify-failed" when it did not. The mixed-type value keeps the program on Brevi's own
# requests (dynamic path).
import sys

import requests

try:
    r = requests.get(sys.argv[1], timeout=30)
    print("ok" if r.status_code < 500 else "status " + str(r.status_code))
except requests.exceptions.SSLError as e:
    print("verify-failed" if "CERTIFICATE_VERIFY_FAILED" in str(e) else "ssl-error " + str(e))
v = 1
v = "one"
print(v)
