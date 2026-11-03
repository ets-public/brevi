import json
from json.decoder import scanstring, JSONDecodeError
s = 'x "a\\nb\\u00e9" [1, {"k": 2.5}] tail'
print(scanstring(s, 3))
dec = json.JSONDecoder()
v, end = dec.raw_decode(s, 15)
print(v, end, s[end:])
print(dec.raw_decode('{"a": [true, null]}'), dec.decode(' [1] '))
for bad in [(s, 1), ('"abc', 1)]:
    try:
        print(scanstring(*bad))
    except JSONDecodeError as e:
        print("JSONDecodeError:", e)
for bad in [' 1', 'nope', '']:
    try:
        print(dec.raw_decode(bad))
    except ValueError as e:
        print("ValueError:", e, e.pos)
