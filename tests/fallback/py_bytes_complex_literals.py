# Bytes and complex literals are not in the typed build: the file goes to the dynamic build, which prints what
# python3 prints. Before, `rb"..."` was read as a str in the typed build and printed `a\d` for b'a\\d', and
# `2j` failed with "expected end of line, found j".
print(rb"a\d", Rb"q", bR"z")
print(b"\x41\0", len(b"\012"))
z = 2j
print(z, 1 + 2J, .5j, 1e3j, abs(3 + 4j))
