# tools/fuzz.py fuzz class 3 (reduced): chr() of an int outside 0..0x10FFFF is ValueError, even past 64 bits
def case_0():
    for v in (65, 0x10FFFF, 0x110000, -1, 2**63, -2**63 - 1, 10**20):
        try:
            print(repr(chr(v)) if v < 128 else len(chr(v)))
        except ValueError as e:
            print("ValueError", e)
case_0()
