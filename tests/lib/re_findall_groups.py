# re.findall gives the whole matches with no group, the group with one, and tuples with two or more.
import re

PAIR = re.compile(r"(\w)=(\d+)")
print(re.findall(r"\d+", "a1 b22 c333"))
print(re.findall(r"[a-z](\d)", "a1 b22 c333"))
print(re.findall(r"(\w)(\d)", "a1 b2"))
print(re.findall(r"(?P<k>\w+):(?:\s*)(\d+)", "x: 1, yy:22"))
print(PAIR.findall("a=1 b=22"))
for k, v in PAIR.findall("c=3"):
    print(k, int(v) + 1)
print(re.findall(r"[(](\d)[)]", "(1) (2)"))
