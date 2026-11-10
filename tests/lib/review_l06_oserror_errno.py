e=OSError(2,"no file","x.txt")
print(e.args, e.errno, e.strerror, e.filename, str(e))
print(repr(e), type(e).__name__)
print(OSError(2,"no file").args, OSError(2,"no file").errno, OSError(2,"no file").strerror, OSError(2,"no file").filename, str(OSError(2,"no file")))
print(OSError("msg").args, OSError("msg").errno, OSError("msg").strerror, OSError("msg").filename, str(OSError("msg")))
print(OSError().args, OSError().errno, OSError().strerror, OSError().filename, str(OSError()))
print(OSError(2).args, OSError(2).errno, OSError(2).strerror, OSError(2).filename, str(OSError(2)))
print(OSError(1,"denied","x").errno, type(OSError(1,"denied","x")).__name__, repr(OSError(1,"denied","x")))
print(OSError(110,"timed out","x").errno, type(OSError(110,"timed out","x")).__name__, str(OSError(110,"timed out","x")))
print(OSError(5,"io").errno, type(OSError(5,"io")).__name__, repr(OSError(5,"io")))
e=OSError(2,"no file","x.txt")
e.errno=3
print(e.errno, e.strerror, e.filename, str(e), repr(e), e.args)
e=OSError(2,"no file",None)
print(e.args, e.errno, e.strerror, e.filename, str(e), repr(e))
e=OSError(2,"no file","x.txt","ignored","other")
print(e.args, e.errno, e.strerror, e.filename, e.filename2, str(e))
try:
    OSError(errno=2)
except TypeError as t:
    print("TypeError", str(t))
class MyErr(OSError): pass
m=MyErr(2,"no file","x.txt")
print(m.args, m.errno, m.strerror, m.filename, str(m), repr(m), type(m).__name__)
print(vars(OSError(2,"no file","x.txt")))
# The errno -> subclass table: each platform's errno.h numbers, as CPython's errnomodule.c maps them.
for code, want in [(1,"PermissionError"), (2,"FileNotFoundError"), (3,"ProcessLookupError"), (4,"InterruptedError"),
                   (10,"ChildProcessError"), (11,"BlockingIOError"), (13,"PermissionError"), (16,"OSError"),
                   (17,"FileExistsError"), (20,"NotADirectoryError"), (21,"IsADirectoryError"), (32,"BrokenPipeError"),
                   (35,"OSError"), (103,"ConnectionAbortedError"), (104,"ConnectionResetError"),
                   (108,"BrokenPipeError"), (110,"TimeoutError"), (111,"ConnectionRefusedError"),
                   (114,"BlockingIOError"), (115,"BlockingIOError"), (5,"OSError"), (122,"OSError")]:
    e = OSError(code, "m")
    print(code, type(e).__name__, e.errno, type(e).__name__ == want)
# CPython 3.13's ImportError/AttributeError/NameError take their C fields by keyword, defaults None.
e = ImportError("x", name="foo", path="p")
print(e.args, e.name, e.path, str(e), repr(e), vars(e))
print(ImportError("x").name, ImportError("x").path, ImportError().name, ImportError().path)
e = AttributeError("x", name="a", obj="o")
print(e.args, e.name, e.obj, str(e), repr(e), vars(e))
print(AttributeError().name, AttributeError().obj)
e = NameError("x", name="n")
print(e.args, e.name, str(e), repr(e), vars(e))
print(NameError().name, ModuleNotFoundError("x", name="m", path="q").name, ModuleNotFoundError("x", name="m", path="q").path)
e = ImportError("x", name="foo")
del e.name
print(e.name, hasattr(e, "name"), vars(e))
e = AttributeError("x", obj="o")
del e.obj
print(e.obj, hasattr(e, "obj"))
e = NameError(name="n")
del e.name
print(e.name, hasattr(e, "name"))
e = ImportError("x", name="foo")
e.name = "q"; e.path = None
print(e.name, e.path, vars(e))
class MyImp(ImportError): pass
m = MyImp("x", name="a", path="b")
print(m.args, m.name, m.path, repr(m), type(m).__name__)
print(UnboundLocalError(name="u").name, UnboundLocalError().name)
for f in [lambda: ImportError("x", name="a", path="p", extra=1),
          lambda: ImportError("x", name="a", path="p", extra=1, foo=2),
          lambda: AttributeError("x", name="a", obj="o", extra=1),
          lambda: NameError(name="a", extra=1),
          lambda: NameError(foo=1),
          lambda: ValueError(x=1),
          lambda: OSError(x=1)]:
    try:
        f()
        print("no error")
    except TypeError as t:
        print(str(t))
