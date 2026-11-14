"""The script's docstring."""
# A program's own modules: repr, __file__, __name__, __doc__ and __package__ as CPython 3.13 gives them
# (the path is the file's absolute path, as the script's own __file__).
import modattrs
from modattrs import helper, plain

here = __file__[: -len("module_attrs.py")]
sep = here[-1]
path = here + "modattrs" + sep + "helper.py"
print(helper.__file__ == path, plain.__file__ == here + "modattrs" + sep + "plain.py")
print(repr(helper) == "<module 'modattrs.helper' from " + repr(path) + ">")
print(str(helper) == repr(helper), f"{helper}" == repr(helper), [helper] == [helper])
print(repr(modattrs) == "<module 'modattrs' from " + repr(here + "modattrs" + sep + "__init__.py") + ">")
print(helper.__name__, plain.__name__, modattrs.__name__)
print(repr(helper.__doc__), plain.__doc__, modattrs.__doc__)
print(repr(helper.__package__), repr(plain.__package__), repr(modattrs.__package__))
print(helper.own())
m = plain
print(m.__name__, m.__doc__, m.__package__, m.__file__ == plain.__file__, getattr(m, "__file__") == plain.__file__)
print(hasattr(m, "__path__"), hasattr(m, "__file__"))
try:
    m.__path__
except AttributeError as e:
    print(e)
print(__doc__, __package__, __name__)


def f():
    return __doc__


class C:
    x = __doc__


print(f(), C.x)
