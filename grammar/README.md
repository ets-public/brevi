# Brevi grammar for constrained decoding

Two copies of one grammar, for engines that only let a model write text the grammar allows:

- `brevi.lark`: Lark syntax. This is the source.
- `brevi.gbnf`: llama.cpp GBNF, made from the Lark file by `tools/lark_to_gbnf.py`. Do not edit it by hand.

The grammar checks each line, not indentation. A line ending in `:` opens a block, and how deep each block goes is left to the compiler. It accepts all 71 programs in `tests/run` and all 1,370 Brevi solutions in `compat/baseline.txt` (checked 31 Oct 2026), except the bodies of `rust fn` and `py fn`, which are foreign code. It rejects things like Python's `def` and `import`, unclosed brackets, `x = = 2` and two statements on one line.

## Use

llama.cpp:

```
llama-cli -m model.gguf --grammar-file grammar/brevi.gbnf -p "..."
```

With `llama-server`, send the file's text as `"grammar"` in a `/completion` request.

Lark-based engines (Lark itself, llguidance, and vLLM's `guidance` backend) take `brevi.lark`. Use an Earley parser: the grammar is ambiguous on purpose, since it only needs to accept or reject. Only Lark has been tested; other engines may need small changes.

## Checks

- `python3 tools/grammar_check.py` parses every known-good program with `brevi.lark` (needs `pip install lark`). With file names, it parses just those.
- `python3 tools/lark_to_gbnf.py --check` fails when `brevi.gbnf` is out of date.

CI runs both, the first on `tests/run` (the full set takes about 11 minutes on 4 cores). The GBNF copy was also checked by turning it back into a Lark grammar: `python3 tools/grammar_check.py --gbnf`. When the language gains syntax, add it to `brevi.lark`, then run `tools/lark_to_gbnf.py`.
