try:
    print(1 + "a")
except TypeError:
    print("caught")


def later():
    return helper() + 1


def helper():
    return 1


print(later())
