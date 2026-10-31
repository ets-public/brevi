"""Writes editor rule files from skills/brevi/SKILL.md, so Cursor, GitHub Copilot, Windsurf and Cline users
get the same instructions as Agent Skills users. Run after changing SKILL.md: python3 skills/gen_rules.py
Copy the file for your editor into your own project at the path given in skills/rules/README.md."""
import os, re
here = os.path.dirname(os.path.abspath(__file__))
src = open(os.path.join(here, "brevi", "SKILL.md")).read()
m = re.match(r"---\n(.*?)\n---\n", src, re.S)
desc = re.search(r"^description: (.*)$", m[1], re.M)[1]
body = src[m.end():].lstrip()
out = {
    "cursor/brevi.mdc": f"---\ndescription: {desc}\nglobs: \"**/*.bv\"\nalwaysApply: false\n---\n\n{body}",
    "copilot/brevi.instructions.md": f"---\napplyTo: \"**/*.bv\"\n---\n\n{body}",
    "windsurf/brevi.md": f"---\ntrigger: glob\nglobs: \"**/*.bv\"\n---\n\n{body}",
    "cline/brevi.md": body,
}
for path, text in out.items():
    p = os.path.join(here, "rules", path); os.makedirs(os.path.dirname(p), exist_ok=True)
    open(p, "w").write(text)
open(os.path.join(here, "rules", "README.md"), "w").write("""# Brevi rules for coding editors

Generated from `../brevi/SKILL.md` by `../gen_rules.py`; edit SKILL.md, not these. Copy the one for your editor into your project:

| Editor | File | Copy to |
| --- | --- | --- |
| Claude Code and other Agent Skills tools | `../brevi/` | `.claude/skills/brevi/` |
| Cursor | `cursor/brevi.mdc` | `.cursor/rules/brevi.mdc` |
| GitHub Copilot | `copilot/brevi.instructions.md` | `.github/instructions/brevi.instructions.md` |
| Windsurf | `windsurf/brevi.md` | `.windsurf/rules/brevi.md` |
| Cline | `cline/brevi.md` | `.clinerules/brevi.md` |

Each applies to `.bv` files and tells the model to read `brevi guide` once, then work from the compiler's JSON errors.
""")
print("wrote", len(out), "rule files")
