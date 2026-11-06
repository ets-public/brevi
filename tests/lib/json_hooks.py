import json, io
class P:
    def __init__(self, x, y):
        self.x, self.y = x, y
class E(json.JSONEncoder):
    def default(self, o):
        if isinstance(o, P):
            return {"x": o.x, "y": o.y}
        return super().default(o)
print(json.dumps({"z": P(1, 2)}, cls=E))
print(json.dumps([P(3, 4)], cls=E, indent=2, sort_keys=True))
print(E().encode([P(5, 6), "é"]), E(ensure_ascii=False).encode("é"))
e = json.JSONEncoder(indent=3, separators=(",", "="))
print(e.indent, repr(e.item_separator), repr(e.key_separator), e.encode({"a": [1]}))
try:
    json.dumps({"q": object()}, cls=E)
except TypeError as ex:
    print(ex)
try:
    json.dumps(1, foo=1)
except TypeError as ex:
    print(ex)
try:
    json.loads("1", foo=1)
except TypeError as ex:
    print(ex)
a = []
a.append(a)
for f in [lambda: json.dumps(a), lambda: json.dumps(float("nan"), allow_nan=False), lambda: json.loads("\ufeff1"), lambda: json.loads(b"\xef\xbb\xbf[1]"), lambda: json.loads("[1,]"), lambda: json.loads('{"a":1,}'), lambda: json.loads('"a\x01"', strict=False), lambda: json.dumps({"a": float("inf")}, allow_nan=False), lambda: json.loads(b'[1, "x"]'), lambda: json.loads(b'\xff\xfe[\x001\x00]\x00'), lambda: json.loads(b'\x00[\x002\x00]'), lambda: json.loads(5)]:
    try:
        print(repr(f()))
    except Exception as ex:
        print(type(ex).__name__, ex)
print(json.loads('{"a": 1.5, "b": 2}', parse_float=str, parse_int=lambda t: int(t) * 10))
print(json.loads('[{"a": 1}, {"b": {"c": 2}}]', object_hook=lambda o: sorted(o)))
print(json.loads('{"a": 1, "a": 2, "b": 3}', object_pairs_hook=list))
print(json.loads('[NaN, -Infinity]', parse_constant=lambda c: "C" + c))
class D(json.JSONDecoder):
    def __init__(self, **kw):
        super().__init__(object_hook=self.hook, **kw)
    def hook(self, o):
        return {k.upper(): v for k, v in o.items()}
print(json.loads('{"a": {"b": 1}}', cls=D))
d = json.JSONDecoder(parse_int=float)
print(d.decode(" [1, 2] "), d.raw_decode('[3] xx'))
try:
    d.decode("[1] x")
except json.JSONDecodeError as ex:
    print(ex)
buf = io.StringIO()
json.dump({"p": P(7, 8)}, buf, cls=E)
print(buf.getvalue())
print(json.load(io.StringIO('{"k": 1.25}'), parse_float=lambda t: t + "!"))
print(json.dumps({"t": (1, 2)}, default=str), json.dumps({1: 2, 2.5: 3, True: 4, None: 5}))
print(json.dumps({"b": 1, "a": 2}, sort_keys=True, separators=(",", ":")))
try:
    json.dumps({1: 1, "a": 2}, sort_keys=True)
except TypeError as ex:
    print(ex)
b = [1]
print(json.dumps([b, b]))
print(json.JSONEncoder, json.JSONDecoder, isinstance(E(), json.JSONEncoder))
