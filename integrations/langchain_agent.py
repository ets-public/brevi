"""A LangChain agent that writes and runs Brevi through `brevi mcp`.
pip install langchain langchain-anthropic langchain-mcp-adapters; needs brevi on PATH and ANTHROPIC_API_KEY.
usage: python langchain_agent.py "Read numbers from stdin and print their median"
"""
import asyncio, sys
from langchain.agents import create_agent
from langchain_mcp_adapters.client import MultiServerMCPClient

PROMPT = "Write programs in Brevi. Read the guide tool once, then use run or check until the program works. Fix only the lines an error names."


async def main(task):
    client = MultiServerMCPClient({"brevi": {"command": "brevi", "args": ["mcp"], "transport": "stdio"}})
    tools = await client.get_tools()  # guide, check, run, test, patch
    agent = create_agent("anthropic:claude-sonnet-5-5", tools, system_prompt=PROMPT)
    result = await agent.ainvoke({"messages": [{"role": "user", "content": task}]})
    print(result["messages"][-1].content)


if __name__ == "__main__":
    asyncio.run(main(" ".join(sys.argv[1:]) or "Print the first 10 primes"))
