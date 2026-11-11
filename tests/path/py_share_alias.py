# Imported under another name: not followed, so the call may change xs.
from _share_push import push as add

xs = [1]
view = xs
add(xs, 2)
print(view)
