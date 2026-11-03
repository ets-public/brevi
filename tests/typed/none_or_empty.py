def f(x):
    if x > 2:
        return None
    return [] if x == 1 else [x]

def main():
    for i in range(4):
        r = f(i)
        print(i, r is None, r)

main()
