# An error's message reads its args when str() runs, and a list can change before that: the typed build takes
# only str, int, float and bool args, so this file goes to the dynamic build.
class Bad(Exception):
    def __init__(self, xs):
        super().__init__(xs)
        self.xs = xs


xs = [1, 2]
try:
    raise Bad(xs)
except Bad as e:
    xs.append(3)
    print(e)
