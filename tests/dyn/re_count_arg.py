# sub(count=) and split(maxsplit=) read as an index, as CPython's _sre does (re.I is 2), with its errors.
import warnings
warnings.simplefilter("ignore")
import re
class I:
    def __index__(self): return 1
for c in [re.I, True, False, 0, -1, 2, I(), 10**30, -10**30, 2.0, "1", None]:
    try: print(repr(re.compile("a").sub("b", "aaa", c)), repr(re.split("a", "xaxax", c)))
    except Exception as e: print(type(e).__name__, e)
