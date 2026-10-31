import re
import sys

REF_RE = re.compile(r"\$\{([^}]*)\}")


class Cycle(Exception):
    pass


def main():
    text = sys.stdin.read().split("\n")
    sep = text.index("---") if "---" in text else len(text)
    ini_lines = text[:sep]
    query_lines = text[sep + 1:]

    sections = {}  # name -> {key: raw value}
    sections["default"] = {}
    current = "default"
    seen = set()
    for raw in ini_lines:
        line = raw.strip()
        if line == "" or line[0] in ";#":
            continue
        if line.startswith("[") and line.endswith("]"):
            current = line[1:-1].strip()
            sections.setdefault(current, {})
            seen.add(current)
            continue
        if "=" not in line:
            continue
        k, v = line.split("=", 1)
        sections.setdefault(current, {})[k.strip()] = v.strip()

    def has_key(sec, key):
        return sec in sections and key in sections[sec]

    def find_qualified(name):
        # Try every dot as a section/key split point.
        for i, ch in enumerate(name):
            if ch == ".":
                s, k = name[:i], name[i + 1:]
                if has_key(s, k):
                    return (s, k)
        return None

    def target_of(ref, sec):
        q = find_qualified(ref)
        if q is not None:
            return q
        if has_key(sec, ref):
            return (sec, ref)
        if has_key("default", ref):
            return ("default", ref)
        return None

    def resolve(sec, key, stack):
        raw = sections[sec][key]

        def repl(m):
            t = target_of(m.group(1), sec)
            if t is None:
                return ""
            if t in stack:
                raise Cycle()
            return resolve(t[0], t[1], stack + [t])

        return REF_RE.sub(repl, raw)

    def value_for(sec, key):
        try:
            return resolve(sec, key, [(sec, key)])
        except Cycle:
            return "<cycle>"

    out = []
    for raw in query_lines:
        q = raw.strip()
        if q == "":
            continue
        if q == "sections":
            names = sorted(s for s in sections if sections[s])
            out.append(" ".join(names))
            continue
        if q.endswith(".*"):
            sec = q[:-2]
            if sec not in seen and sec not in sections:
                out.append("<missing>")
                continue
            for key in sorted(sections.get(sec, {})):
                out.append("%s=%s" % (key, value_for(sec, key)))
            continue
        found = find_qualified(q)
        if found is None:
            out.append("<missing>")
        else:
            out.append(value_for(found[0], found[1]))
    sys.stdout.write("\n".join(out) + ("\n" if out else ""))


if __name__ == "__main__":
    main()
