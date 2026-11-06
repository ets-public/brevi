import atexit
import sys
def bye(n, tag="t"):
    print("bye", n, tag)

atexit.register(bye, 1, tag="k")
@atexit.register
def later():
    print("later", atexit._ncallbacks())
atexit.register(bye, 2)
atexit.register(bye, 3)
atexit.unregister(bye)
atexit.register(bye, 4)
try:
    atexit.register(5)
except TypeError as e:
    print(e)
print("main")
sys.exit(3)
