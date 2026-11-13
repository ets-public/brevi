# `a is b` on values other than None, True, False and Ellipsis needs object identity, which the typed build
# does not have: its lexer reads `is` as `==`, so it answered True for two equal lists and for a str built at
# run time. The guard sends the file to the dynamic build.
a = [1]
b = [1]
print(a is b, a is a, a is not b)
s = "hello world"
t = " ".join(["hello", "world"])
print(s == t, s is t)
