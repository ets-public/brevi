# The official MCP time server (mcp-server-time on PyPI), started as its own `mcp-server-time` command starts
# it. run.py puts the package's server.py, unchanged, beside this file: `pip install mcp-server-time==2026.11.4`.
import argparse
import asyncio

from server import serve

ap = argparse.ArgumentParser(description="give a model the ability to handle time queries and timezone conversions")
ap.add_argument("--local-timezone", type=str, help="Override local timezone")
asyncio.run(serve(ap.parse_args().local_timezone))
