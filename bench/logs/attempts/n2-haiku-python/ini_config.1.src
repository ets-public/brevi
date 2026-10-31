import sys
import re

REF_RE = re.compile(r"\$\{([^}]*)\}")


class Cycle(Exception):
    pass


def main():
    data = sys.stdin.read().split("\n")
    if "---" in data:
        k = data.index("---")
        ini_lines = data[:k]
        queries = data[k + 1:]
    else:
        ini_lines = data
        queries = []

    sections = {}  # name -> {key: value}
    current = None
    for raw in ini_lines:
        s = raw.strip()
        if s == "":
            continue
        if s.startswith(";") or s.startswith("#"):
            continue
        if s.startswith("[") and s.endswith("]"):
            current = s[1:-1].strip()
            sections.setdefault(current, {})
            continue
        if "=" in s:
            key, value = s.split("=", 1)
            key = key.strip()
            value = value.strip()
            if current is None:
                current = "default"
                sections.setdefault(current, {})
            sections[current][key] = value

    def find_key(ref, cur):
        """Return (section, key) for a reference, or None if missing."""
        if "." in ref:
            # try section.key, longest section name first
            best = None
            for sec in sections:
                if ref.startswith(sec + ".") and ref[len(sec) + 1:] in sections[sec]:
                    if best is None or len(sec) > len(best[0]):
                        best = (sec, ref[len(sec) + 1:])
            if best is not None:
                return best
        if ref in sections.get(cur, {}):
            return (cur, ref)
        if ref in sections.get("default", {}):
            return ("default", ref)
        return None

    def resolve(sec, key, stack):
        if (sec, key) in stack:
            raise Cycle()
        raw = sections[sec][key]
        new_stack = stack + ((sec, key),)

        def repl(m):
            target = find_key(m.group(1), sec)
            if target is None:
                return ""
            return resolve(target[0], target[1], new_stack)

        return REF_RE.sub(repl, raw)

    def value_of(sec, key):
        try:
            return resolve(sec, key, ())
        except Cycle:
            return "<cycle>"

    out = []
    for q in queries:
        q = q.strip()
        if q == "":
            continue
        if q == "sections":
            names = sorted(name for name, kv in sections.items() if kv)
            out.append(" ".join(names))
            continue
        if q.endswith(".*"):
            sec = q[:-2]
            if sec not in sections:
                out.append("<missing>")
                continue
            for key in sorted(sections[sec]):
                out.append("%s=%s" % (key, value_of(sec, key)))
            continue
        found = None
        for name in sections:
            if q.startswith(name + ".") and q[len(name) + 1:] in sections[name]:
                if found is None or len(name) > len(found[0]):
                    found = (name, q[len(name) + 1:])
        if found is None:
            if "." in q:
                sec, _, key = q.rpartition(".")
                if sec in sections and key in sections[sec]:
                    found = (sec, key)
        if found is None:
            out.append("<missing>")
        else:
            out.append(value_of(found[0], found[1]))

    if out:
        sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
