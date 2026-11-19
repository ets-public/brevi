# A function replaces the item m came from before m changes.
def reset(d):
    d["a"] = []


def main():
    q = {"a": [1], "b": [2]}
    m = q["a"]
    reset(q)
    m.append(1)
    print(q, m)


main()
