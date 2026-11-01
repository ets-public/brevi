"""An OpenAI Agents SDK agent that writes and runs Brevi through `brevi mcp`.
pip install openai-agents; needs brevi on PATH and a key for the model you choose.
usage: python openai_agents.py "Read numbers from stdin and print their median"
"""
import asyncio, sys
from agents import Agent, Runner
from agents.mcp import MCPServerStdio

PROMPT = "Write programs in Brevi. Read the guide tool once, then use run or check until the program works. Fix only the lines an error names."


async def main(task):
    async with MCPServerStdio(params={"command": "brevi", "args": ["mcp"]}, name="brevi") as brevi:
        agent = Agent(name="Brevi coder", instructions=PROMPT, mcp_servers=[brevi])
        result = await Runner.run(agent, task)
        print(result.final_output)


if __name__ == "__main__":
    asyncio.run(main(" ".join(sys.argv[1:]) or "Print the first 10 primes"))
