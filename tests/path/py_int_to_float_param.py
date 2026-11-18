# An int passed where other calls pass a float stays an int in Python (3, not 3.0): the typed build would
# print it as a float, so the file goes to the dynamic build.
class Box:
    def __init__(self, w):
        self.w = w


def show(x):
    print(x)


show(2.5)
show(3)
print(Box(2.5).w, Box(3).w)
