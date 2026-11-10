from collections import Counter


def test_copy_subclass():
    class MyCounter(Counter):
        pass

    c = MyCounter("abracadabra")
    d = c.copy()
    print(type(d))
    print(type(d) is MyCounter)
    print(MyCounter.__qualname__)
    print(MyCounter.__name__)


test_copy_subclass()
