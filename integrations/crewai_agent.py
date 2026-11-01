"""A CrewAI agent that writes and runs Brevi through `brevi mcp`.
pip install "crewai-tools[mcp]"; needs brevi on PATH and ANTHROPIC_API_KEY.
usage: python crewai_agent.py "Read numbers from stdin and print their median"
"""
import sys
from crewai import Agent, Crew, Task
from crewai_tools import MCPServerAdapter
from mcp import StdioServerParameters

task_text = " ".join(sys.argv[1:]) or "Print the first 10 primes"
with MCPServerAdapter(StdioServerParameters(command="brevi", args=["mcp"])) as tools:
    coder = Agent(
        role="Brevi programmer",
        goal="Write a Brevi program that works on the first run",
        backstory="Reads the guide tool once, then uses run or check and fixes only the lines an error names.",
        tools=tools,
        llm="anthropic/claude-sonnet-5-5",
    )
    job = Task(description=task_text, expected_output="The finished Brevi program and its output", agent=coder)
    print(Crew(agents=[coder], tasks=[job]).kickoff())
