# Run with no locale set: python3 reads text as UTF-8 (PEP 538/540), and so must the CPython a native build embeds (#211).
import json
import pathlib

print(json.loads((pathlib.Path(__file__).parent / "d.json").read_text())["a"])
