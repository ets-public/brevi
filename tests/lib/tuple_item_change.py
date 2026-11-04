# A tuple holds the list itself, not a copy, so a later item that changes the list shows in the tuple.
q = [1, 2]
r = (q, q.pop())
print(r)
s = [1, 2, 3]
a, c = s.pop(), s.pop()
print(a, c, s)
