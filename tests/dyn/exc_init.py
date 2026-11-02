class SqlErr(Exception):
    def __init__(self, msg):
        Exception.__init__(self, msg)
        self.msg = msg
class B:
    def __init__(self, x):
        self.x = x
class C(B):
    def __init__(self):
        B.__init__(self, 5)
print(C().x)
try:
    raise SqlErr('error: x')
except SqlErr as e:
    print(e, e.msg)
