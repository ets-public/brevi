from enum import Flag, IntFlag, auto

class Perm(Flag):
    READ = auto()
    WRITE = auto()
    EXEC = auto()

rw = Perm.READ | Perm.WRITE
print(rw in Perm, Perm.READ in rw, ~Perm.READ in Perm, rw)
print(5 in Perm, 0 in Perm, 7 in Perm, 9 in Perm, -1 in Perm, 8 in Perm)
print(1.0 in Perm, 3.0 in Perm, False in Perm, True in Perm, 'x' in Perm)
print(Perm.READ | Perm.EXEC in Perm)
print(Perm(5), Perm(5) in Perm)
print(Perm.READ | Perm.EXEC in rw)
try:
    print(1 in rw)
except TypeError as e:
    print(type(e).__name__, e)

class MyInt(int):
    pass
print(MyInt(1) in Perm, MyInt(5) in Perm, MyInt(9) in Perm)

class P2(IntFlag):
    A = 1
    B = 2
print(5 in P2, 8 in P2, 3.0 in P2, 1.0 in P2, False in P2, 0 in P2)
