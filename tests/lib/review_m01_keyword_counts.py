def f(a, b, *, c=0, d=0): return (a,b,c,d)
def g(a, b=1, *, c, d=0): return (a,b,c)
def h(a, /, b, **kw): return (a, b, kw)
def q(a, b, /, c=5): return (a,b,c)
for fn, args, kw in [(f,(1,2,3),{'c':1}),(f,(1,2,3),{'c':1,'d':2}),(f,(1,2,3),{}),(g,(1,2,3),{'c':1}),(g,(1,2,3,4),{'c':1,'d':2}),(f,(1,2,3),{'b':1}),
  (h,(),{'a':1,'b':2}),(h,(1,),{'a':1,'b':2}),(q,(1,),{'b':2}),(q,(1,2),{'a':1,'b':2}),(q,(1,2,3,4),{}),(q,(1,2,3,4),{'a':1}),(q,(1,2),{'c':9}),
  (h,(1,2,3),{}),(f,(1,2,3),{'x':1})]:
    try: print(fn(*args, **kw))
    except TypeError as e: print("TE", e)
