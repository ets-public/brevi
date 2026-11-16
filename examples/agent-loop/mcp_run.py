"""Send one program to `brevi mcp` over stdio, as an agent harness would, and print the reply.

Usage: python3 mcp_run.py main.py STDIN
Uses only the standard library. The server sandboxes the program by default and checks the answer
(`verify` is on by default for `run`).
"""
import json
import subprocess
import sys

path, stdin = sys.argv[1], sys.argv[2]
code = open(path, encoding="utf-8").read()
srv = subprocess.Popen(["brevi", "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)


def rpc(i, method, params):
    srv.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i, "method": method, "params": params}) + "\n")
    srv.stdin.flush()
    while True:
        msg = json.loads(srv.stdout.readline())
        if msg.get("id") == i:
            return msg


rpc(1, "initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "demo", "version": "1"}})
srv.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
reply = rpc(2, "tools/call", {"name": "run", "arguments": {"code": code, "stdin": stdin}})
print("".join(c.get("text", "") for c in reply["result"]["content"]))
srv.stdin.close()
srv.wait()
