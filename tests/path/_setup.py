# Statements CPython runs once, when the module is first imported.
LOG = []
print("setting up")
for n in range(3):
    LOG.append(n * n)


def total():
    return sum(LOG)
