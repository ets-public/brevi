# Brevi

Brevi is a programming language for AI models to write. It reads like Python, compiles to Rust, and reports every error as one short JSON line with a suggested fix.

```
npm install -g brevi
brevi guide          # the language guide, for the model to read once
brevi run app.bv
brevi mcp            # MCP server over stdio: guide, check, run, test, patch
```

Brevi builds programs with `rustc`, so a Rust toolchain must be on the PATH (https://rustup.rs).
Docs: https://brevi-lang.org

