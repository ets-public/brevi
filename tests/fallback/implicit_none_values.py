# Using the None of a def with no return as a value, and a value on some paths of a last if/else.
def plain(x):
    x + 1


def test_value(x):
    x > 1


def other(x):
    if x > 0:
        return x
    else:
        x + 5


print(plain(1) is None, [plain(1), plain(2)])
v = plain(1)
print(v)
if test_value(3):
    print("yes")
else:
    print("no")
print(other(1), other(-1))
