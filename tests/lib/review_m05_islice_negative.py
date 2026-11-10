from itertools import islice, permutations, combinations, combinations_with_replacement, product

try:
    print(list(islice([1,2,3],-1)))
except ValueError as e:
    print("VE",e)
try:
    print(list(permutations([1,2],-1)))
except ValueError as e:
    print("VE",e)

def check(label, fn):
    try:
        print(label, list(fn()))
    except Exception as e:
        print(label, type(e).__name__, e)

check("islice stop -1", lambda: islice([1,2,3], -1))
check("islice stop -2", lambda: islice([1,2,3], -2))
check("islice stop -1 3arg", lambda: islice([1,2,3], None, -1))
check("islice stop -2 3arg", lambda: islice([1,2,3], None, -2))
check("islice start -1", lambda: islice([1,2,3], -1, None))
check("islice start -2", lambda: islice([1,2,3], -2, None))
check("islice step -1", lambda: islice([1,2,3], 0, 3, -1))
check("islice step 0", lambda: islice([1,2,3], 1, 2, 0))
check("islice huge stop", lambda: islice([1,2,3], -(2**100)))
check("islice huge start", lambda: islice([1,2,3], 2**100, None))
check("islice huge step", lambda: islice([1,2,3], 0, 2, 2**100))
check("islice none bounds", lambda: islice([1,2,3], None, None, None))
check("islice normal", lambda: islice([1,2,3,4,5], 1, 5, 2))
check("islice missing args", lambda: islice([1,2,3]))
check("islice too many args", lambda: islice([1,2,3], 0, 3, 1, 9))
check("islice keyword", lambda: islice([1,2,3], stop=1))
check("permutations r -1", lambda: permutations([1,2], -1))
check("permutations r -5", lambda: permutations([1,2], -5))
check("permutations r None", lambda: permutations([1,2], None))
check("permutations huge r", lambda: permutations([1,2], -(2**100)))
check("combinations r -1", lambda: combinations([1,2], -1))
check("combinations_with_replacement r -1", lambda: combinations_with_replacement([1,2], -1))
check("product repeat -1", lambda: product([1,2], repeat=-1))
check("product repeat 0", lambda: product([1,2], repeat=0))
check("product repeat huge", lambda: product([1,2], repeat=-(2**100)))
