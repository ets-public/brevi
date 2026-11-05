def f(v):
    try:
        return int(v)
    except ValueError as e:
        print("bad", e)
        return -1


def g(v):
    for x in v:
        try:
            int(x)
        except ValueError as err:
            continue
    # Python deletes an except ... as name when the handler ends, also by continue
    try:
        print(err)
    except NameError as ne:
        print("NameError", ne)


print(f("x"), f("3"))
g(["a"])
try:
    raise KeyError("k")
except KeyError as top:
    pass
print("top" in globals())
