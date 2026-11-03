# `v is True` / `v is False` on JSON values: only the bool itself (1 == True, but 1 is not True).
import json
vals = json.loads('[true, false, 1, 0, "true", null, 1.0]')
for v in vals:
    print(v is True, v is False, v is not True, v is not False, v == True)
flag = True
print(flag is True, flag is not False)
