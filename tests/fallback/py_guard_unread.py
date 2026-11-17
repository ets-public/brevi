# The guard cannot read a lone surrogate in a string next to an f-string, which the typed parser takes: that must
# not count as nothing to refuse.
x = 1
s = "\ud800" f"{x}"
print("made")
print({3, -1, 5})
