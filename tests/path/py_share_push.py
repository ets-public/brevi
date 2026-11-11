# The module's function changes the list, which another name still shows: not the typed build.
from _share_push import push

xs = [1]
view = xs
push(xs, 2)
print(view)
