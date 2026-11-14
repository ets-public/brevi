"""Helper module.

    Indented second line."""
X = 1


def own():
    return __name__, __doc__, __package__, __file__.endswith("helper.py")
