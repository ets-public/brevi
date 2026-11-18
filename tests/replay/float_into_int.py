# A parameter that is an int at one call and a float at another: the typed build stops at the float and
# `brevi run` gives Python's answer from the dynamic build.
def add(a, b):
    return a + b


print(add(1, 2), add(0.5, 1))
