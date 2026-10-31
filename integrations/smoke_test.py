"""Checks that each framework can reach `brevi mcp`: lists the tools and runs one program. No model or key needed.
usage: python smoke_test.py   (skips a framework that is not installed; Vercel AI is checked with node if ai is installed)
"""
import asyncio, os, subprocess, sys

PROGRAM = 'n = 3\nprint("n is {n}")'
WANT = "n is 3"
TOOLS = ["guide", "check", "run", "test", "patch"]
results = {}


async def langchain():
    from langchain_mcp_adapters.client import MultiServerMCPClient
    c = MultiServerMCPClient({"brevi": {"command": "brevi", "args": ["mcp"], "transport": "stdio"}})
    tools = {t.name: t for t in await c.get_tools()}
    out = await tools["run"].ainvoke({"code": PROGRAM})
    return list(tools) == TOOLS and WANT in str(out)


async def openai_agents():
    from agents.mcp import MCPServerStdio
    async with MCPServerStdio(params={"command": "brevi", "args": ["mcp"]}) as s:
        names = [t.name for t in await s.list_tools()]
        out = await s.call_tool("run", {"code": PROGRAM})
        return names == TOOLS and WANT in out.content[0].text


def crewai():
    from crewai_tools import MCPServerAdapter
    from mcp import StdioServerParameters
    with MCPServerAdapter(StdioServerParameters(command="brevi", args=["mcp"])) as tools:
        run = [t for t in tools if t.name == "run"][0]
        return [t.name for t in tools] == TOOLS and WANT in run.run(code=PROGRAM)


def vercel():
    js = ("import {createMCPClient} from '@ai-sdk/mcp';import {Experimental_StdioMCPTransport as T} from '@ai-sdk/mcp/mcp-stdio';"
          "const c=await createMCPClient({transport:new T({command:'brevi',args:['mcp']})});const t=await c.tools();"
          f"const r=await t.run.execute({{code:{PROGRAM!r}}},{{toolCallId:'x',messages:[]}});"
          "console.log(Object.keys(t).join(','), r.content[0].text);await c.close();")
    p = subprocess.run(["node", "--input-type=module", "-e", js], capture_output=True, text=True, timeout=60)
    if "Cannot find" in p.stderr:
        raise ImportError(p.stderr)
    return p.stdout.startswith(",".join(TOOLS)) and WANT in p.stdout


for name, f in [("langchain", langchain), ("openai-agents", openai_agents), ("crewai", crewai), ("vercel-ai", vercel)]:
    try:
        r = f()
        results[name] = asyncio.run(r) if asyncio.iscoroutine(r) else r
    except ImportError:
        results[name] = None
for name, ok in results.items():
    print(f"{name}: {'skipped (not installed)' if ok is None else 'ok' if ok else 'FAILED'}")
sys.exit(1 if False in results.values() else 0)
