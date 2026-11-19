# Where the bytes of a built binary go: ELF sections, then code and data by crate, then the largest symbols.
#
#   python3 bench/showcase/size.py BINARY [--top N] [--json]
#
# Needs binutils (readelf, nm), so Linux only. Sections come from `readelf -SW`; the crate split comes from
# `nm -S -C` symbol sizes. A symbol is charged to the first of Brevi's own crates its name mentions (brevi_prog is
# the compiled program, brevi_dyn the dynamic runtime, brevi_rt the typed runtime), so `Vec<brevi_dyn::Obj>::drop`
# counts as runtime code, not as alloc (brevi_lib holds the library copies the program imports, compiled from
# src/dynpy/py: argparse, pydantic, the mcp SDK, ...); a symbol that names none of them is charged to the crate it starts with.
# Bytes in a section that no symbol covers (string literals, jump tables, padding) are listed as "unnamed".
# run.py calls breakdown() for every showcase it builds and writes the result to results.json.
import json
import os
import re
import subprocess
import sys

OWN = ("brevi_prog", "brevi_lib", "brevi_dyn", "brevi_rt")
# sections that are loaded and run (code, read-only data, unwind tables) against what strip=symbols removes
SYMBOL_SECTIONS = (".symtab", ".strtab")


def sections(binary):
    out = subprocess.run(["readelf", "-SW", binary], capture_output=True, text=True, check=True).stdout
    res = {}
    for line in out.splitlines():
        # [Nr] Name Type Address Off Size ES Flg Lk Inf Al
        m = re.match(r"\s*\[\s*\d+\]\s+(\S+)\s+(\S+)\s+[0-9a-f]+\s+[0-9a-f]+\s+([0-9a-f]+)", line)
        if m and m.group(2) != "NULL" and m.group(2) != "NOBITS":
            res[m.group(1)] = res.get(m.group(1), 0) + int(m.group(3), 16)
    return res


def crate_of(name):
    first = None
    for c in OWN:
        i = name.find(c + "::")
        if i >= 0 and (first is None or i < first[0]):
            first = (i, c)
    if first:
        return first[1]
    m = re.match(r"[<&*\s]*(?:dyn |mut |const )*([A-Za-z_][A-Za-z0-9_]*)::", name)
    if m:
        return m.group(1)
    return "(no crate: C, libc or linker symbols)"


def symbols(binary):
    # nm -S lists sizes; -C demangles Rust and C++ names. Each address is counted once (aliases share one).
    out = subprocess.run(["nm", "-S", "-C", "--size-sort", binary], capture_output=True, text=True).stdout
    seen = {}
    for line in out.splitlines():
        parts = line.split(None, 3)
        if len(parts) < 4:
            continue
        addr, size, kind, name = parts
        if addr in seen:
            continue
        seen[addr] = (int(size, 16), kind.lower(), name)
    return list(seen.values())


def breakdown(binary, top=15):
    secs = sections(binary)
    syms = symbols(binary)
    code, data = {}, {}
    code_named = data_named = 0
    for size, kind, name in syms:
        c = crate_of(name)
        if kind in ("t", "w"):
            code[c] = code.get(c, 0) + size
            code_named += size
        elif kind in ("r", "d", "b", "v"):
            if kind == "b":  # zero-filled at start, takes no file bytes
                continue
            data[c] = data.get(c, 0) + size
            data_named += size
    text = secs.get(".text", 0)
    ro = sum(v for k, v in secs.items() if k.startswith((".rodata", ".data")))
    biggest = sorted(syms, reverse=True)[:top]
    return {
        "file_bytes": os.path.getsize(binary),
        "sections": dict(sorted(secs.items(), key=lambda kv: -kv[1])),
        "symbol_table_bytes": sum(secs.get(s, 0) for s in SYMBOL_SECTIONS),
        "debug_bytes": sum(v for k, v in secs.items() if k.startswith(".debug")),
        "code_by_crate": dict(sorted(code.items(), key=lambda kv: -kv[1])),
        "code_unnamed": max(text - code_named, 0),
        "data_by_crate": dict(sorted(data.items(), key=lambda kv: -kv[1])),
        "data_unnamed": max(ro - data_named, 0),
        "largest_symbols": [{"bytes": s, "kind": k, "name": n[:160]} for s, k, n in biggest],
    }


def mb(n):
    return "%6.2f MB" % (n / 1e6)


def show(b, top=15):
    lines = ["file %s" % mb(b["file_bytes"]).strip(), "", "sections:"]
    for k, v in b["sections"].items():
        if v >= 10_000:
            lines.append("  %-24s %s" % (k, mb(v)))
    lines.append("  (symbol table %s, debug info %s: what strip=symbols / strip=debuginfo remove)" % (
        mb(b["symbol_table_bytes"]).strip(), mb(b["debug_bytes"]).strip()))
    for title, key, rest in (("code (.text) by crate:", "code_by_crate", "code_unnamed"),
                             ("data (.rodata, .data*) by crate:", "data_by_crate", "data_unnamed")):
        lines += ["", title]
        for k, v in b[key].items():
            if v >= 10_000:
                lines.append("  %-40s %s" % (k, mb(v)))
        lines.append("  %-40s %s" % ("unnamed (literals, tables, padding)", mb(b[rest])))
    lines += ["", "largest symbols:"]
    for s in b["largest_symbols"][:top]:
        lines.append("  %s  %s  %s" % (mb(s["bytes"]), s["kind"], s["name"]))
    return "\n".join(lines)


def main():
    args = sys.argv[1:]
    top = int(args[args.index("--top") + 1]) if "--top" in args else 15
    paths = [a for i, a in enumerate(args) if not a.startswith("--") and (i == 0 or args[i - 1] != "--top")]
    if len(paths) != 1:
        sys.exit("usage: size.py BINARY [--top N] [--json]")
    b = breakdown(paths[0], top)
    print(json.dumps(b, indent=1) if "--json" in args else show(b, top))


if __name__ == "__main__":
    main()
