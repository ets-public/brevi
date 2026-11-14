print("init subpk.one")
import subpk
B = 2
try:
    subpk.one
except AttributeError as e:
    print(e)
