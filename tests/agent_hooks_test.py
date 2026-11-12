#!/usr/bin/env python3
"""Tests for tools/agent_hooks/context_ceiling.py, tools/agent_hooks/install.py and tools/session_tokens.py.
Fixed transcripts and hook inputs only: no network, no Claude Code install. usage: python3 tests/agent_hooks_test.py"""
import json, os, subprocess, sys, tempfile, unittest

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
HOOK = os.path.join(ROOT, "tools", "agent_hooks", "context_ceiling.py")
INSTALL = os.path.join(ROOT, "tools", "agent_hooks", "install.py")
TOKENS = os.path.join(ROOT, "tools", "session_tokens.py")


def transcript(d, name, contexts):
    p = os.path.join(d, name)
    with open(p, "w") as f:
        for i, c in enumerate(contexts):
            u = {"input_tokens": 2, "cache_creation_input_tokens": 1000, "cache_read_input_tokens": c - 1002, "output_tokens": 50}
            for _ in range(2):  # a request's usage is copied onto each of its content records
                f.write(json.dumps({"type": "assistant", "requestId": f"r{i}", "gitBranch": "b", "message": {"usage": u}}) + "\n")
    return p


class Hook(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp()
        self.env = {k: v for k, v in os.environ.items() if k not in ("CLAUDE_CODE_CHILD_SESSION", "BREVI_CONTEXT_CEILING")}
        self.env["TMPDIR"] = self.d

    def run_hook(self, ctx, tool, tin, worker=True, extra=None):
        path = transcript(self.d, f"s{ctx}.jsonl", [90000, ctx])
        env = dict(self.env, **({"CLAUDE_CODE_CHILD_SESSION": "1"} if worker else {}))
        payload = {"tool_name": tool, "tool_input": tin, "transcript_path": path, "session_id": f"s{ctx}", **(extra or {})}
        r = subprocess.run([sys.executable, HOOK], input=json.dumps(payload), capture_output=True, text=True, env=env)
        self.assertEqual(r.returncode, 0, r.stderr)
        return json.loads(r.stdout)["hookSpecificOutput"] if r.stdout.strip() else None

    def denied(self, out):
        return bool(out) and out.get("permissionDecision") == "deny"

    def test_below_advisory_silent(self):
        self.assertIsNone(self.run_hook(120000, "Bash", {"command": "ls"}))

    def test_advisory_once(self):
        out = self.run_hook(260000, "Bash", {"command": "ls"})
        self.assertIn("additionalContext", out)
        self.assertIsNone(self.run_hook(260000, "Bash", {"command": "ls"}))

    def test_ceiling_refuses_work(self):
        for tool, tin in [("Bash", {"command": "cargo build --release"}), ("Read", {"file_path": "/x"}),
                          ("Bash", {"command": "git status > /tmp/x"}), ("Bash", {"command": "git log | head"}),
                          ("Bash", {"command": "git status; cat src/main.rs"}), ("Bash", {"command": "echo $(git log)"}),
                          ("Write", {"file_path": "/home/user/brevi/src/x.rs"}),
                          ("Write", {"file_path": "/mnt/project-files/handoffs/../secrets.md"}), ("Agent", {})]:
            self.assertTrue(self.denied(self.run_hook(310000, tool, tin)), (tool, tin))

    def test_ceiling_allows_handoff(self):
        for tool, tin in [("Bash", {"command": "git add -u && git commit -m x && git push -u origin b"}),
                          ("Bash", {"command": "gh pr view 1"}),
                          ("Bash", {"command": "python3 tools/session_tokens.py --record --pr 5 --task \"x y\""}),
                          ("Write", {"file_path": "/mnt/project-files/handoffs/re.md"}),
                          ("Edit", {"file_path": "/tmp/claude/memory/team/silo/x.md"}),
                          ("mcp__hearthbot__post_message", {}), ("ToolSearch", {})]:
            self.assertFalse(self.denied(self.run_hook(310000, tool, tin)), (tool, tin))

    def test_not_a_worker_never_judged(self):
        self.assertIsNone(self.run_hook(900000, "Bash", {"command": "cargo build"}, worker=False))

    def test_subagent_judged_from_its_own_transcript(self):
        sess = os.path.join(self.d, "main.jsonl")
        open(sess, "w").close()
        sub = os.path.join(self.d, "main", "subagents")
        os.makedirs(sub)
        transcript(sub, "agent-a1.jsonl", [310000])
        payload = {"tool_name": "Bash", "tool_input": {"command": "ls"}, "transcript_path": sess, "agent_id": "a1"}
        r = subprocess.run([sys.executable, HOOK], input=json.dumps(payload), capture_output=True, text=True, env=self.env)
        self.assertTrue(self.denied(json.loads(r.stdout)["hookSpecificOutput"]))

    def test_fails_open(self):
        env = dict(self.env, CLAUDE_CODE_CHILD_SESSION="1")
        for stdin in ["not json", json.dumps({"tool_name": "Bash", "transcript_path": "/nonexistent"})]:
            r = subprocess.run([sys.executable, HOOK], input=stdin, capture_output=True, text=True, env=env)
            self.assertEqual((r.returncode, r.stdout), (0, ""))


class Install(unittest.TestCase):
    def test_idempotent_and_keeps_other_settings(self):
        d = tempfile.mkdtemp()
        s = os.path.join(d, "settings.json")
        json.dump({"model": "x", "hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "other"}]}]}}, open(s, "w"))
        for _ in range(2):
            subprocess.run([sys.executable, INSTALL, "--settings", s], check=True, capture_output=True)
        cfg = json.load(open(s))
        self.assertEqual(cfg["model"], "x")
        cmds = [h["command"] for g in cfg["hooks"]["PreToolUse"] for h in g["hooks"]]
        self.assertEqual(len(cmds), 2)
        self.assertIn("other", cmds)


class Tokens(unittest.TestCase):
    def test_counts_each_request_once_and_records(self):
        home = tempfile.mkdtemp()
        proj = os.path.join(home, ".claude", "projects", "p")
        os.makedirs(proj)
        transcript(proj, "sess1.jsonl", [100000, 200000])
        out = os.path.join(home, "rec")
        env = dict(os.environ, HOME=home, CLAUDE_CODE_SESSION_ID="sess1")
        subprocess.run([sys.executable, TOKENS, "--record", "--pr", "7", "--out", out], check=True, capture_output=True, env=env)
        rec = json.load(open(os.path.join(out, os.listdir(out)[0])))
        self.assertEqual(rec["calls"], 2)
        self.assertEqual(rec["cache_read_input_tokens"], 100000 - 1002 + 200000 - 1002)
        self.assertEqual((rec["first_context"], rec["max_context"], rec["prs"]), (100000, 200000, [7]))
        r = subprocess.run([sys.executable, TOKENS, "--report", out], check=True, capture_output=True, text=True, env=env)
        self.assertIn("1 merged PRs", r.stdout)


if __name__ == "__main__":
    unittest.main()
