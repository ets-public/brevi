import sys

def encode(text):
    out = []
    i = 0
    while i < len(text):
        j = i
        while j < len(text) and text[j] == text[i]:
            j += 1
        out.append(f"{text[i]}{j - i}")
        i = j
    return "".join(out)

def decode(code):
    out = []
    i = 0
    while i < len(code):
        ch = code[i]
        i += 1
        j = i
        while j < len(code) and code[j].isdigit():
            j += 1
        if j > i:
            out.append(ch * int(code[i:j]))
        i = j
    return "".join(out)

for line in sys.stdin.read().splitlines():
    if not line:
        continue
    op, _, rest = line.partition(" ")
    if op == "e":
        print(encode(rest))
    elif op == "d":
        print(decode(rest))
