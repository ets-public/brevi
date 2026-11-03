tables = {}
def val(s):
    if s[0] == "'":
        return s[1:-1]
    return int(s)
def gettable(name):
    return tables[name]
def run(cmd, name):
    if cmd == "c":
        tables[name] = ({"a": (0, "INT")}, ["a"], [])
        return
    t = gettable(name)
    if cmd == "i":
        t[2].append([val("1"), val("'p'")])
    elif cmd == "u":
        for r in t[2]:
            r[1] = val("'z'")
    else:
        print(t[2])
def main():
    for c in ["c", "i", "i", "u", "p"]:
        run(c, "x")
main()
