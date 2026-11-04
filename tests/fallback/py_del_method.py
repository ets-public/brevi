class F:
    n = 0
    def __init__(self):
        F.n += 1
    def __del__(self):
        F.n -= 1
x = F()
x = None
print(F.n)
