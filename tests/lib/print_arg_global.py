# A later print argument calls a function that changes a global: the earlier argument shows the change, while
# an int read before the call keeps its value.
log = [0]
hits = 0

def note(x):
    global hits
    hits += 1
    log.append(x)
    return hits

print(log, hits, note(4))
print(hits, note(5), log)
