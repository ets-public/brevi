# re.findall with no group or one group stays typed (a list of strings).
import re

WORD = re.compile(r"[a-z]+")
print(re.findall(r"\d+", "a1 b22 c333"))
print(re.findall(r"[a-z](\d)", "a1 b22"))
print(WORD.findall("hi there 42"))
print(re.findall(r"(?:ab)+(c)", "ababc abc"))
