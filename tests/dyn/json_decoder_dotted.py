import json.decoder

s = '"a\\nb" rest'
val, end = json.decoder.scanstring(s, 1)
print(repr(val), end)
try:
    json.decoder.scanstring('"abc', 1)
except ValueError:
    print("unterminated")
