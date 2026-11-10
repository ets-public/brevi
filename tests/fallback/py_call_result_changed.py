# `get_bucket("a").append(1)` with get_bucket returning mem[name]: Python changes mem's list; the typed
# build changed a copy and lost the 1. Found by the copy-versus-alias sweep (9 Nov 2026).
mem = {}
def get_bucket(name):
    if name not in mem:
        mem[name] = []
    return mem[name]
get_bucket("a").append(1)
b = get_bucket("a")
b.append(2)
print(mem)
