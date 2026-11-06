# under --sandbox the network is the caller's to grant: --allow-net names the hosts a program may reach
import httpx, requests
print(httpx.get("http://127.0.0.1:18080/echo").status_code)
print(requests.get("http://127.0.0.1:18080/echo").status_code)
try:
    httpx.get("http://localhost:18080/echo")
except Exception as e:
    print(type(e).__name__, e)
try:
    requests.get("http://localhost:18080/echo")
except Exception as e:
    print(type(e).__name__, e)
