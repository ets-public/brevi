# casefold uses Unicode full case folding (ß -> ss) and isdigit accepts non-ASCII decimal and
# superscript digits, in both the typed and dynamic builds.
print("straße".casefold() == "strasse".casefold(), "ß".casefold(), "١".isdigit())
# nearby casefold cases: capital sharp s, the dotted I, final sigma, and Greek / ligature expansions
print("ẞ".casefold(), "İ".casefold(), "Σ".casefold(), "ς".casefold(), "ᾈ".casefold(), "ﬀ".casefold())
# nearby isdigit cases: superscripts, circled digits, a numeric-but-not-digit fraction, and other scripts
print("²".isdigit(), "³".isdigit(), "①".isdigit(), "½".isdigit(), "١".isdigit(), "".isdigit(), "a١".isdigit())
print("𑄶".isdigit(), "᭘".isdigit(), "𐄂".isdigit())
# isnumeric adds Numeric_Type=Numeric characters: fractions, number words and Unihan numerals
print("١".isnumeric(), "²".isnumeric(), "½".isnumeric(), "一".isnumeric(), "Ⅷ".isnumeric(), "".isnumeric(), "12½".isnumeric())
# isdecimal is the Nd decimal digits only
print("١".isdecimal(), "²".isdecimal(), "½".isdecimal(), "3".isdecimal(), "一".isdecimal(), "".isdecimal())
print("٣²".isdigit(), "٣½".isdigit(), "𐌢".isnumeric())
