import re
def outer():
    codes = []
    def sc(m):
        codes.append(m.group(0))
        return "x"
    alias = codes
    print(re.sub("a", sc, "aba"))
    print(alias)
outer()
