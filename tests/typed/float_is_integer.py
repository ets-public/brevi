x = 2.0
y = 2.5
print(x.is_integer(), y.is_integer(), float("inf").is_integer(), (7).is_integer())
import json
v = json.loads("[1.0, 2.5, 3]")
print([a.is_integer() for a in v])
