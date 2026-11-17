# String escapes in the typed build: NUL and octal escapes (core test_int's "123\0"), the u"" prefix.
s = "123\0"
print(len(s), repr(s))
print(repr("a\012b"), repr("\101"), repr("x\07y"), repr('\00'), repr("\1"), repr("\x41"))
print(u"hi", U'a\tb', len(u"\0"))
try:
    int("123\0")
except ValueError as e:
    print(e)
