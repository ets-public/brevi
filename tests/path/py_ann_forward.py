# An annotation CPython evaluates when the def runs names a class defined further down: NameError there, so the
# typed build, which never evaluates annotations, must not take the file (src/ann_guard.rs).
print("start")
def area(s: Square) -> int:
    return s.n * s.n
class Square:
    def __init__(self, n):
        self.n = n
print(area(Square(3)))
