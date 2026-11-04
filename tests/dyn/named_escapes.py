# \N{NAME} escapes: names, any case, aliases, in every kind of Python string; raw strings and \\N stay text.
n = 3
print("\N{BULLET} item")
print('\N{EM DASH} single')
print("\N{bullet} \N{Latin Small Letter E With Acute}")
print(ord("\N{NBSP}"), ord("\N{ZWJ}"), ord("\N{LF}"))
print(f"\N{BULLET} {n} \N{DEGREE SIGN}")
print(f'{n}\N{BULLET}{n}')
print("""\N{SNOWMAN}
\N{GREEK SMALL LETTER PI}""")
print("\N{LEFT CURLY BRACKET}x\N{RIGHT CURLY BRACKET}")
print(f"\N{LEFT CURLY BRACKET}{n}\N{RIGHT CURLY BRACKET}")
print("a\N{LATIN SMALL LETTER B}c" == "abc")
print(len("\N{GRINNING FACE}"))
print(r"\N{BULLET}")
print(r"\N{not a name}")
print("\\N{BULLET}")
print(f"{n:\N{BULLET}>5}")
print(rf"\N{n}")
