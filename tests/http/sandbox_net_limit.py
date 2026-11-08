# --limit net=20kb: the program stops once it has sent and received 20 KB over the network.
import httpx

for i in range(2000):
    httpx.get("http://127.0.0.1:18080/echo")
print("not reached")
