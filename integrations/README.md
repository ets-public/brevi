# Brevi in agent frameworks

Each framework reaches Brevi through its own MCP client and `brevi mcp`, so nothing here needs a package of its own. The server gives the agent seven tools: `guide`, `check`, `run`, `test`, `patch`, `edit` and `show`. The agent sends its program, plain Python, as text once; the server keeps it, so fixes send only the changed function or lines, and the agent needs no files. Install `brevi` first (see the main README), so it is on PATH.

| Framework | Example | Install |
| --- | --- | --- |
| LangChain | [langchain_agent.py](langchain_agent.py) | `pip install langchain langchain-anthropic langchain-mcp-adapters` |
| OpenAI Agents SDK | [openai_agents.py](openai_agents.py) | `pip install openai-agents` |
| CrewAI | [crewai_agent.py](crewai_agent.py) | `pip install "crewai-tools[mcp]"` |
| Vercel AI SDK | [vercel_ai.mjs](vercel_ai.mjs) | `npm i ai @ai-sdk/mcp @ai-sdk/anthropic` |

The connection is a few lines in each framework:

```python
# LangChain
tools = await MultiServerMCPClient({"brevi": {"command": "brevi", "args": ["mcp"], "transport": "stdio"}}).get_tools()
# OpenAI Agents SDK
async with MCPServerStdio(params={"command": "brevi", "args": ["mcp"]}) as brevi:
    agent = Agent(name="coder", mcp_servers=[brevi])
# CrewAI
with MCPServerAdapter(StdioServerParameters(command="brevi", args=["mcp"])) as tools:
    agent = Agent(role="coder", tools=tools, ...)
```

```js
// Vercel AI SDK
const client = await createMCPClient({ transport: new Experimental_StdioMCPTransport({ command: 'brevi', args: ['mcp'] }) });
const tools = await client.tools();
```

Any other MCP client works the same way: the command is `brevi`, with the argument `mcp`. A sandboxed agent can use the Docker image instead: `docker run --rm -i brevi mcp`.

## Tested (31 Oct 2026)

`python smoke_test.py` lists the tools and runs a program through each framework that is installed, with no model or key needed. It passed with langchain-mcp-adapters 0.3.2, openai-agents 0.24.0, crewai-tools 1.15.33, and ai 7.0 with @ai-sdk/mcp 2.0. The LangChain and Vercel AI examples were also run end to end with Claude Sonnet 5.5: each agent wrote a program, ran it and reported the right output. The OpenAI Agents and CrewAI examples have not been run against a model.
