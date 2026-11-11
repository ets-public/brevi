class A:
    x: int
class B(A):
    pass
class C:
    pass
print(B().__annotations__)
print(B.__annotations__)
try:
    print(C().__annotations__)
except AttributeError as e:
    print("AttributeError", e)
print(C.__annotations__)
print(C().__annotations__)
print("__annotations__" in B.__dict__)
