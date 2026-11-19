# Calls giving different numbers of args to an __init__ with a default: CPython's message is the args each call
# gave ('a', then ('b', 3)), so the file goes to the dynamic build.
class Bad(Exception):
    def __init__(self, msg, line=None):
        self.msg = msg


for args in (("a",), ("b", 3)):
    try:
        if len(args) == 1:
            raise Bad(args[0])
        raise Bad(args[0], args[1])
    except Bad as e:
        print(e)
