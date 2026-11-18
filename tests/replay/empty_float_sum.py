# sum() of no floats is the int 0 in Python, not 0.0.
ys = [1.5, 2.5]
zs = [y for y in ys if y > 5]
print(sum(ys), sum(ys, 0))
print(sum(zs))
