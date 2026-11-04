# CPython evaluates every print argument before writing any, so a later argument that changes a list shows
# in an earlier one. The typed build reads each argument as it goes.
def grow(v):
    v.append(9)
    return len(v)

xs = [1]
print(xs, grow(xs))
print(len(xs), grow(xs), xs, sep="|")
