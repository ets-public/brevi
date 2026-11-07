"""A Claude agent whose code runs in a platform sandbox under Brevi.

    pip install anthropic   (plus the platform's SDK: e2b, daytona or modal)
    export ANTHROPIC_API_KEY=...
    python templates/agent.py --platform docker "How many primes are there below 10 million?"
    python templates/agent.py --platform e2b --allow-net api.github.com "Which of ets-public's repos has the most stars?"

--platform is one of docker, e2b, daytona, modal, cloudflare. The model writes Python and calls run_python;
the program is compiled by Brevi and runs in that platform's sandbox with only the grants and limits given
here on the command line (or in wrangler.jsonc for Cloudflare).
"""
import argparse, importlib.util, os, sys

import anthropic

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from brevi_tool import TOOL, Grants  # noqa: E402

SYSTEM = ("You answer questions by writing and running Python with the run_python tool. Run code rather than "
          "guess; when it fails, change only what the error names and run again. Finish with a short answer.")


def platform(name):
    spec = importlib.util.spec_from_file_location(f"brevi_{name}", os.path.join(HERE, name, "sandbox.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.Sandbox


def agent(task, box, model="claude-sonnet-5-5", max_turns=10, log=print):
    client = anthropic.Anthropic(api_key=os.environ.get("ANTHROPIC_API_KEY") or os.environ.get("BENCH_ANTHROPIC_KEY"))
    messages = [{"role": "user", "content": task}]
    for _ in range(max_turns):
        msg = client.messages.create(model=model, max_tokens=8000, system=SYSTEM, tools=[TOOL], messages=messages)
        messages.append({"role": "assistant", "content": msg.content})
        calls = [b for b in msg.content if b.type == "tool_use"]
        if not calls:
            return "".join(b.text for b in msg.content if b.type == "text")
        results = []
        for c in calls:
            r = box.run(c.input["code"], c.input.get("stdin", ""))
            log(f"--- run_python: exit {r.exit_code}" + (f", stopped at its {r.limit} limit" if r.limit else ""))
            results.append({"type": "tool_result", "tool_use_id": c.id, "content": r.for_model(), "is_error": r.exit_code != 0})
        messages.append({"role": "user", "content": results})
    return "(stopped after %d turns)" % max_turns


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("task")
    ap.add_argument("--platform", default="docker", choices=["docker", "e2b", "daytona", "modal", "cloudflare"])
    ap.add_argument("--model", default="claude-sonnet-5-5")
    ap.add_argument("--allow", default="", help="read,write,net,native: what programs may do (default nothing)")
    ap.add_argument("--allow-net", default="", help="hosts programs may connect to, comma-separated")
    ap.add_argument("--limit", default="cpu=10,mem=512,time=20,output=256,file=16", help="brevi --limit")
    a = ap.parse_args()
    split = lambda s: [x for x in s.split(",") if x]
    limits = dict(kv.split("=", 1) for kv in split(a.limit))
    grants = Grants(allow=split(a.allow), allow_net=split(a.allow_net), limits=limits)
    Sandbox = platform(a.platform)
    with (Sandbox() if a.platform == "cloudflare" else Sandbox(grants)) as box:
        print(agent(a.task, box, model=a.model))


if __name__ == "__main__":
    main()
