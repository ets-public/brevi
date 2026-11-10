def f():
    try:
        del nope
    except UnboundLocalError as e:
        return str(e)
print(f())

def f2():
    x = 1
    del x
    try:
        del x
    except UnboundLocalError as e:
        return type(e).__name__ + ': ' + str(e)
    return 'done'
print(f2())

def f3():
    x = 1
    del x
    try:
        return x
    except UnboundLocalError as e:
        return str(e)
print(f3())

def f4():
    def h():
        return y
    try:
        del y
    except Exception as e:
        return type(e).__name__ + ': ' + str(e)
    return 'done'
print(f4())

def f5():
    def h():
        nonlocal y
        try:
            del y
        except Exception as e:
            return type(e).__name__ + ': ' + str(e)
    y = 2
    del y
    return h()
print(f5())

gv = 5
def f6():
    global gv
    del gv
f6()
try:
    print(gv)
except Exception as e:
    print(type(e).__name__ + ': ' + str(e))

def f7():
    global gv2
    try:
        del gv2
    except Exception as e:
        return type(e).__name__ + ': ' + str(e)
print(f7())

def f8():
    try:
        del len
    except Exception as e:
        return type(e).__name__ + ': ' + str(e)
print(f8())

def f9():
    try:
        class C:
            del z
    except NameError as e:
        return str(e)
    return 'done'
print(f9())

def f10():
    try:
        class C:
            x = 1
            del x
            del x
    except NameError as e:
        return type(e).__name__ + ': ' + str(e)
    return 'done'
print(f10())

def f11():
    x = 1
    try:
        class C:
            del x
    except NameError as e:
        return type(e).__name__ + ': ' + str(e)
    return 'done'
print(f11())

def f12():
    class C:
        x = 1
        del x
    try:
        C.x
    except AttributeError as e:
        return str(e)
    return 'has'
print(f12())
