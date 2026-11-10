def tryit(f, *a):
    try:
        print(repr(f(*a)))
    except Exception as e:
        print(type(e).__name__, e)
for v in (65, 0x10FFFF, 0x110000, -1, 2**31, 2**63 - 1, 2**63, -2**63, -2**63 - 1, 10**20, -10**20, 2**64):
    tryit(chr, v)
try:
    chr(10**20)
except ValueError as e:
    print("VE", e)
except OverflowError as e:
    print("OE", e)
try:
    chr(2**64)
except (IndexError, ValueError) as e:
    print("caught", type(e).__name__)
except OverflowError as e:
    print("OE", e)
