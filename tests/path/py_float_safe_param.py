# An int passed to a float parameter that is only used where 3 and 3.0 give the same answer (next to a
# float, in /, compared, formatted as a float) keeps the typed build.
def scaled(r):
    return 3.14159 * r * r > 10.0


def half(v):
    return v / 2


def show(v):
    return f"{v:.2f}"


def big(v):
    return v > 2


print(scaled(2), scaled(2.5), half(3), half(2.5), show(3), show(2.5), big(3), big(2.5))
