"""Tests for bench/deepseek.py: the message mapping, and a whole e2e.solve run against a fake DeepSeek server
(no network, no key). Run: python3 bench/test_deepseek.py
"""
import http.server, json, os, sys, tempfile, threading, unittest
from pathlib import Path

B = Path(__file__).resolve().parent
sys.path.insert(0, str(B))
import deepseek, e2e  # noqa: E402

FIZZ = (B / "solutions" / "sonnet-python" / "fizzbuzz.py").read_text()
BROKEN = FIZZ.replace('"Fizz"', '"Fiz"')


def reply(content=None, calls=(), reasoning="thinking...", finish="tool_calls", hit=0, miss=100, out=50):
    return {"choices": [{"index": 0, "finish_reason": finish, "message": {
        "role": "assistant", "content": content, "reasoning_content": reasoning,
        "tool_calls": [{"id": f"call_{i}", "type": "function", "function": {"name": n, "arguments": json.dumps(a)}} for i, (n, a) in enumerate(calls)] or None}}],
        "usage": {"prompt_tokens": hit + miss, "completion_tokens": out, "prompt_cache_hit_tokens": hit, "prompt_cache_miss_tokens": miss,
                  "completion_tokens_details": {"reasoning_tokens": out // 2}}}


class Fake(http.server.BaseHTTPRequestHandler):
    script, seen = [], []

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        Fake.seen.append((self.path, self.headers["Authorization"], body))
        if Fake.script and Fake.script[0] == 429:
            Fake.script.pop(0)
            self.send_response(429); self.end_headers(); self.wfile.write(b'{"error":"busy"}')
            return
        data = json.dumps(Fake.script.pop(0)).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        # DeepSeek sends keep-alive blank lines before a slow non-streamed answer
        self.wfile.write(b"\n\n" + data)

    def log_message(self, *a):
        pass


class DeepSeekTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Fake)
        threading.Thread(target=cls.srv.serve_forever, daemon=True).start()
        cls.client = deepseek.Client(api_key="sk-test", base_url=f"http://127.0.0.1:{cls.srv.server_port}", max_retries=2)
        deepseek.time.sleep = lambda s: None  # retries without waiting

    @classmethod
    def tearDownClass(cls):
        cls.srv.shutdown()

    def setUp(self):
        Fake.script, Fake.seen = [], []

    def test_mapping(self):
        system = [{"type": "text", "text": "be brief", "cache_control": {"type": "ephemeral"}}]
        msgs = [{"role": "user", "content": "task"},
                {"role": "assistant", "content": [deepseek.Block(type="thinking", thinking="hmm"), deepseek.Block(type="text", text="ok"),
                                                  deepseek.Block(type="tool_use", id="c1", name="check", input={})]},
                {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "c1", "content": "test 1 failed", "is_error": True}]}]
        out = deepseek.to_openai(system, msgs)
        self.assertEqual(out[0], {"role": "system", "content": "be brief"})
        self.assertEqual(out[2]["reasoning_content"], "hmm")
        self.assertEqual(out[2]["tool_calls"][0]["function"], {"name": "check", "arguments": "{}"})
        self.assertEqual(out[3], {"role": "tool", "tool_call_id": "c1", "content": "error: test 1 failed"})
        tools = deepseek.to_openai_tools(e2e.tools("pyb"))
        self.assertEqual([t["function"]["name"] for t in tools], ["write_file", "edit_file", "check"])
        self.assertNotIn("strict", tools[0]["function"])

    def test_usage_and_stop(self):
        m = deepseek.from_openai(reply("done", reasoning=None, finish="stop", hit=900, miss=100, out=40))
        self.assertEqual((m.usage.input_tokens, m.usage.cache_read_input_tokens, m.usage.cache_creation_input_tokens, m.usage.output_tokens), (100, 900, 0, 40))
        self.assertEqual(m.stop_reason, "end_turn")
        self.assertEqual([b.type for b in m.content], ["text"])
        self.assertEqual(deepseek.from_openai(reply(calls=[("check", {})], finish="length")).stop_reason, "max_tokens")

    def test_solve_end_to_end(self):
        """write a broken solution, check (fails), edit it, check (passes), stop: the e2e loop, logs and cost."""
        Fake.script = [
            429,  # one rate limit, retried
            reply(calls=[("write_file", {"path": "main.py", "content": BROKEN}), ("check", {})], miss=1000, out=300),
            reply(calls=[("edit_file", {"path": "main.py", "old": '"Fiz"', "new": '"Fizz"'}), ("check", {})], hit=1200, miss=200, out=80),
            reply("PASS, done.", reasoning=None, finish="stop", hit=1400, miss=50, out=10),
        ]
        work = Path(tempfile.mkdtemp()) / "t1-deepseek-python" / "fizzbuzz"
        r = e2e.solve(self.client, "deepseek-flash", "python", "fizzbuzz", work, "medium")
        self.assertTrue(r["pass"])
        self.assertEqual((r["checks"], r["calls"], r["stop"]), (2, 3, "end_turn"))
        self.assertEqual((r["input"], r["cache_read"], r["cache_write"], r["output"]), (1250, 2600, 0, 390))
        pin, pout, pread = e2e.PRICES["deepseek-flash"]
        self.assertAlmostEqual(r["usd"], round((1250 * pin + 2600 * pread + 390 * pout) / 1e6, 5))
        self.assertIsNotNone(r["usd_to_green"])
        # the request DeepSeek got: key, thinking on with the effort, tool results in order, reasoning sent back
        path, auth, body = Fake.seen[-1]
        self.assertEqual((path, auth, body["model"]), ("/chat/completions", "Bearer sk-test", "deepseek-flash"))
        self.assertEqual((body["thinking"], body["reasoning_effort"]), ({"type": "enabled"}, "medium"))
        roles = [m["role"] for m in body["messages"]]
        self.assertEqual(roles, ["system", "user", "assistant", "tool", "tool", "assistant", "tool", "tool"])
        self.assertTrue(body["messages"][6]["content"].startswith("edited"))
        self.assertTrue(body["messages"][7]["content"].startswith("PASS"))
        self.assertTrue(all(m.get("reasoning_content") for m in body["messages"] if m["role"] == "assistant"))
        self.assertTrue(body["messages"][4]["content"].startswith("error: test"))
        # the saved conversation is plain JSON, as for Claude runs (replay.py reads it)
        conv = json.loads((work / "conversation.json").read_text())
        self.assertEqual(conv[1]["content"][0]["type"], "thinking")

    def test_api_error_is_reported(self):
        Fake.script = [429, 429, 429]
        with self.assertRaisesRegex(RuntimeError, "DeepSeek API 429"):
            self.client.messages.create(model="deepseek-flash", system="s", messages=[{"role": "user", "content": "hi"}])

    def test_verdict_report_only(self):
        """A DeepSeek miss is printed but does not fail the release test."""
        d = Path(tempfile.mkdtemp())
        rows = []
        for model, pyb_usd in (("claude-haiku-5-5", 1.0), ("deepseek-flash", 5.0)):
            for lang, usd in (("pyb", pyb_usd), ("python", 1.0)):
                rows.append({"run": f"v1a-x-{lang}", "model": model, "lang": lang, "task": "t", "pass": True, "usd": usd, "native": True, "checks": 1})
        (d / "v1a.jsonl").write_text("\n".join(json.dumps(r) for r in rows))
        self.assertEqual(e2e.verdict(str(d / "*.jsonl")), 0)
        rows[0]["usd"] = 5.0  # now Haiku misses too
        (d / "v1a.jsonl").write_text("\n".join(json.dumps(r) for r in rows))
        self.assertEqual(e2e.verdict(str(d / "*.jsonl")), 1)


if __name__ == "__main__":
    unittest.main()
