# Python leaves keyword args out of an error's message (str(e) is "" here): the typed build cannot see that
# from __init__, so the file goes to the dynamic build.
class Missing(Exception):
    def __init__(self, key):
        self.key = key


try:
    raise Missing(key="k1")
except Missing as e:
    print(repr(str(e)), e.key)
