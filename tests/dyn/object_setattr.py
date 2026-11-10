# object.__setattr__ and super().__setattr__: past a class's own __setattr__, through properties, argument checks.
# (Not covered: __slots__, which the dynamic build does not enforce on any assignment yet.)
class Plain:
    pass

class Recorder:
    def __init__(self):
        self.log = []
    def __setattr__(self, name, value):
        if name == "log":
            object.__setattr__(self, name, value)
        else:
            self.log.append(name)
            object.__setattr__(self, name, value)

class Frozen:
    def __setattr__(self, name, value):
        raise AttributeError("frozen")

class SubFrozen(Frozen):
    pass

class WithProp:
    def __init__(self):
        object.__setattr__(self, "_v", 0)
    @property
    def p(self):
        return self._v
    @p.setter
    def p(self, v):
        object.__setattr__(self, "_v", v * 10)
    @property
    def ro(self):
        return 42

class Slotted:
    __slots__ = ("a",)

class Upper:
    def __setattr__(self, name, value):
        if isinstance(value, str):
            value = value.upper()
        super().__setattr__(name, value)

class ManualFrozen:
    def __init__(self, x):
        object.__setattr__(self, "x", x)
    def __setattr__(self, name, value):
        raise AttributeError("frozen")

# Case 1
try:
    o = Plain()
    object.__setattr__(o, "a", 1)
    print("case 1:", getattr(o, "a"))
except Exception as e:
    print("case 1:", type(e).__name__)

# Case 2
try:
    r = Recorder()
    r.x = 5
    object.__setattr__(r, "y", 6)
    print("case 2:", r.log, getattr(r, "x"), getattr(r, "y"))
except Exception as e:
    print("case 2:", type(e).__name__)

# Case 3
try:
    s = SubFrozen()
    object.__setattr__(s, "z", 7)
    print("case 3:", getattr(s, "z"))
except Exception as e:
    print("case 3:", type(e).__name__)

# Case 4
try:
    w = WithProp()
    object.__setattr__(w, "p", 3)
    print("case 4a:", getattr(w, "p"))
    try:
        object.__setattr__(w, "ro", 1)
    except Exception as e:
        print("case 4b:", type(e).__name__, e)
except Exception as e:
    print("case 4:", type(e).__name__)

# Case 5
try:
    sl = Slotted()
    object.__setattr__(sl, "a", 9)
    print("case 5a:", getattr(sl, "a"))
except Exception as e:
    print("case 5:", type(e).__name__)

# Case 6
try:
    o6 = Plain()
    object.__setattr__(o6, "__pydantic_fields_set__", {"a"})
    object.__setattr__(o6, "keys", "shadow")
    print("case 6:", getattr(o6, "__pydantic_fields_set__"), getattr(o6, "keys"))
except Exception as e:
    print("case 6:", type(e).__name__)

# Case 7
try:
    o7 = Plain()
    object.__setattr__(o7, 1, 2)
    print("case 7: ok")
except Exception as e:
    print("case 7:", type(e).__name__, e)

# Case 8
try:
    o8 = Plain()
    object.__setattr__(o8, "a")
    print("case 8: ok")
except Exception as e:
    print("case 8:", type(e).__name__, e)

# Case 9
try:
    o9 = Plain()
    for i in range(1000):
        object.__setattr__(o9, f"f{i%7}", i)
    print("case 9:", [getattr(o9, f"f{i}") for i in range(7)])
except Exception as e:
    print("case 9:", type(e).__name__)

# Case 10
try:
    u = Upper()
    u.name = "hello"
    print("case 10:", getattr(u, "name"))
except Exception as e:
    print("case 10:", type(e).__name__)

# Case 11
try:
    mf = ManualFrozen(5)
    print("case 11a:", getattr(mf, "x"))
    try:
        mf.x = 6
    except Exception as e:
        print("case 11b:", type(e).__name__)
except Exception as e:
    print("case 11:", type(e).__name__)
