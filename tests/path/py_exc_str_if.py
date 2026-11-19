# super().__init__ inside an if: the message depends on the branch taken, so the file goes to the dynamic build.
class Odd(Exception):
    def __init__(self, n):
        if n % 2:
            super().__init__(f"odd {n}")
        self.n = n


for n in (3, 4):
    try:
        raise Odd(n)
    except Odd as e:
        print(repr(str(e)), e.n)
