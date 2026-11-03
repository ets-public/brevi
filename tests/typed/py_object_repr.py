# A class without __str__ or __repr__ prints as CPython's default repr (the address differs, the shape does not).
class Plain:
    def __init__(self, a):
        self.a = a


s = str(Plain(1))
print(s.startswith("<__main__.Plain object at 0x"), s.endswith(">"))
print(str([Plain(2)]).startswith("[<__main__.Plain object at 0x"))
