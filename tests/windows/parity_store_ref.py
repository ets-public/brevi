# The reference for parity_store_tls.py: CPython's own default context (load_default_certs, which on Windows
# reads the "CA" and "ROOT" certificate stores) against HOST PORT. Prints what parity_store_tls.py prints.
import socket
import ssl
import sys

host, port = sys.argv[1], int(sys.argv[2])
ctx = ssl.create_default_context()
try:
    with socket.create_connection((host, port), timeout=30) as s:
        with ctx.wrap_socket(s, server_hostname=host):
            print("ok")
except ssl.SSLCertVerificationError:
    print("verify-failed")
print("one")
