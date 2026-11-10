# tools/fuzz.py fuzz class 1 (reduced): 0 ** a negative power raises ZeroDivisionError (Brevi printed inf)
# (error messages are not printed: CPython 3.15 words the 0 ** negative message differently from 3.13)
def case_0():
    for a, b in ((0, -1), (0.0, -2), (0, -1.5), (2, -1)):
        try:
            print(a ** b)
        except ZeroDivisionError as e:
            print("ZeroDivisionError")
    try:
        print(pow(0, -1, 7))
    except ValueError as e:
        print("ValueError")
    print(pow(3, -1, 7))
case_0()
