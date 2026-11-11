# The module keeps the list it is given in its own global: changing it after must show there too.
from _share_keep import keep, kept

xs = [1]
keep(xs)
xs.append(2)
print(kept)
