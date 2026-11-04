"""An agent that fetches live app data with Composio and processes it with Brevi.
pip install composio langchain langchain-anthropic langchain-mcp-adapters; needs brevi on PATH,
ANTHROPIC_API_KEY and COMPOSIO_API_KEY, and a GitHub account connected in Composio for COMPOSIO_USER_ID.
usage: python composio_agent.py "Count the open issues in ets-public/brevi by label"

Composio holds the credentials and makes the network calls. Brevi runs the agent's program in its
sandbox with no capabilities granted: the data arrives as stdin, so the program cannot read files,
open sockets or reach the Composio account itself.
"""
import asyncio, os, sys
from composio import Composio
from langchain.agents import create_agent
from langchain_mcp_adapters.client import MultiServerMCPClient

PROMPT = (
    "Fetch data with the GitHub tool, then write a Brevi program that reads that data as JSON from stdin "
    "and computes the answer. Pass the data to Brevi's run tool as stdin; never paste it into the code. "
    "Read Brevi's guide tool once, then use run until the program works. Fix only the lines an error names."
)


async def main(task):
    composio = Composio()  # reads COMPOSIO_API_KEY
    session = composio.create(
        user_id=os.environ.get("COMPOSIO_USER_ID", "default"),
        toolkits=["github"],
        tools={"github": {"enable": ["GITHUB_LIST_REPOSITORY_ISSUES"]}},
        mcp=True,
    )
    client = MultiServerMCPClient({
        "brevi": {"command": "brevi", "args": ["mcp"], "transport": "stdio"},
        "composio": {"url": session.mcp.url, "headers": session.mcp.headers, "transport": "streamable_http"},
    })
    tools = await client.get_tools()  # Brevi's guide, check, run, test, patch, edit, show, plus the GitHub tool
    agent = create_agent("anthropic:claude-sonnet-5-5", tools, system_prompt=PROMPT)
    result = await agent.ainvoke({"messages": [{"role": "user", "content": task}]})
    print(result["messages"][-1].content)


if __name__ == "__main__":
    asyncio.run(main(" ".join(sys.argv[1:]) or "Count the open issues in ets-public/brevi by label"))
