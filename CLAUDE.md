# Brevi

Every agent session loads this file on every call: keep it short (tools/guide_budget.py caps it at 1,500 bytes).

- Before building or testing, read AGENTS.md (commands, CI, secrets, model-written code). How the compiler fits together: docs/ARCHITECTURE.md, "Where to look when".
- Find code with `python3 tools/code_map.py --grep WORD` (or `code_map.py src/dynpy`): one line per file. Files over 2k lines (prelude.rs, parser.rs, dynpy/emit.rs, ...): grep for the symbol and read that range, never the whole file.
- One task per session. When your PR merges or the task ends: push, write a handoff of under 30 lines to /mnt/project-files/handoffs/<topic>.md, run `python3 tools/session_tokens.py --record --pr N --task "..."`, and end. Past 300k tokens of context, tools/agent_hooks/context_ceiling.py lets only those steps run.
- Run anything over a few minutes (cargo build, conformance) in the background. `tools/ci_local.sh` runs on Adam's PC only (Remote Control), never in a cloud container.
- GitHub or its MCP answering 5xx is an outage, not a blocker: retry later (send_later at 10, 20, 40 min), never hand off over it.
