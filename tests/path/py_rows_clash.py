tables = {}
def gettable(name):
    return tables[name]
def run(cmd, name):
    if cmd == "c":
        tables[name] = ({"a": (0, "INT")}, ["a"], [])
        return
    t = gettable(name)
    if cmd == "i":
        t[2].append([len(t[2]), "p"])
    elif cmd == "u":
        for r in t[2]:
            r[1] = "z"
    else:
        print(t[2])
def main():
    for c in ["c", "i", "i", "u", "p"]:
        run(c, "x")
main()
