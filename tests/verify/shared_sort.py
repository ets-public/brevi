# The typed build once copied a list sorted in a function (the 10 November review, c01); under --sandbox
# `run --verify` checks the typed answer against the dynamic build, which shares the list as Python does.
def smallest(xs):
    xs.sort()
    return xs[0]
data = [3, 1, 2]
print(smallest(data))
print(data)
