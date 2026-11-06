# Brevi cannot build this; with --sandbox it is refused, not run with python3.
class A:
    def f(self):
        s = super
        return s
print(A().f())
