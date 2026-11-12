# #470: a builtin type has only the attributes CPython's type dicts give it (its own, its bases', type's)
for t in (int, float, str, bytes, list, tuple, dict, set, frozenset, bool, BaseException, ValueError, KeyError, range, object, type):
    print(t.__name__, hasattr(t, "zzz"), hasattr(t, "__globals__"), hasattr(t, "__init__"), hasattr(t, "mro"), hasattr(t, "__name__"))
print(hasattr(int, "bit_length"), hasattr(int, "from_bytes"), hasattr(str, "upper"), hasattr(str, "maketrans"), hasattr(list, "append"))
print(hasattr(dict, "fromkeys"), hasattr(dict, "__missing__"), hasattr(list, "__class_getitem__"), hasattr(bool, "bit_count"), hasattr(KeyError, "with_traceback"))
print(hasattr(OSError, "errno"), hasattr(StopIteration, "value"), hasattr(int, "__instancecheck__"), hasattr(int, "__subclasses__"))
for t, n in ((int, "zzz"), (str, "append"), (list, "upper"), (ValueError, "nope")):
    try:
        getattr(t, n)
    except AttributeError as e:
        print("AttributeError:", e)
try:
    int.zzz
except AttributeError as e:
    print("AttributeError:", e)
print(getattr(int, "zzz", "default"))
print(str.upper("ab"), int.bit_length(255), list.__len__([1, 2]), dict.fromkeys("ab", 0))
