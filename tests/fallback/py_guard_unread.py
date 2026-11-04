# The guard cannot read `except*`, which the typed parser takes: that must not count as nothing to refuse.
try:
    raise ExceptionGroup("g", [ValueError(1)])
except* ValueError:
    print("caught")
print({3, -1, 5})
