# __del__ runs when the last reference goes, as in CPython: rebinding, del, clear, a loop target moving on,
# function locals at return, and at exit (with the globals still readable).
class D:
    def __init__(self, n):
        self.n = n
    def __del__(self):
        print("del", self.n)

a = D("a")
a = D("a2")

print("rebound")
b = D("b")
del b
print("deleted")
lst = [D(i) for i in range(3)]
lst.clear()
print("cleared")
for i in range(2):
    x = D(f"loop{i}")
print("looped")
def f():
    t = D("local")
    print("in f")
f()
print("after f")
t = (D("t1"), D("t2"))
t = None
o = D("o")
o.child = D("child")
o = None
log = []
class Uses:
    def __del__(self):
        print("at exit, still sees", len(log), a.n)
u = Uses()

# __del__ that clears a dict during a comparison does not crash (bpo-27945)
class X:
    def __del__(self):
        dict_b.clear()
    def __eq__(self, other):
        dict_a.clear()
        return True
    def __hash__(self):
        return 13
dict_a = {X(): 0}
dict_b = {X(): X()}
print(dict_a == dict_b)
print("end")
