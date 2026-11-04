import json, sys
data = json.loads('[{"labels":[{"name":"bug"}],"meta":{"a":1}},{"labels":[]},{"labels":null,"meta":null}]')
for d in data:
    labels = d.get("labels") or []
    meta = d.get("meta") or {}
    print(len(labels), labels, meta, len(meta))
    for k in meta:
        print("k", k, meta[k])
    names = [l["name"] for l in d["labels"] or []]
    print(names)
