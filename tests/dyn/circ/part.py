import circ
try:
    circ.later
except AttributeError as e:
    print(str(e).replace(circ.__file__, "<file>"))
P = 5
