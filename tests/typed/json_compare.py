# JSON values compared with [], 0, "" and tested for truth, as in Python.
import json
vals = json.loads('[2, 0, true, false, "", [], "x", null]')
def truthy(v):
    if v is None or v is False:
        return False
    if isinstance(v, (int, float)) and v == 0:
        return False
    if v == '' or v == []:
        return False
    return True
for v in vals:
    print(truthy(v), isinstance(v, (int, float)), isinstance(v, int), v == 0, v == '', v == [])
