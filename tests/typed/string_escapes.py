# Python escapes that stand for one character, and repr of control characters.
print(repr("\x41é\U0001F600\101\0\a\x7b"))
print(len("\x00\x1b"), ord("\x7f"), ord('€'))
print(f"{1}\x7b{2}")
print(r"\x41", "\\d")
print(repr('tab\there'), repr(chr(7)), repr("\v\f\b"))
