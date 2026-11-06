import json
def main() -> None:
    d = {"a": 1.0, "b": float("inf")}
    try:
        print(json.dumps(d, allow_nan=False))
    except ValueError as e:
        print("VE", e)
    print(json.dumps(d, check_circular=False, sort_keys=True))
    print(json.dumps([1, 2], default=str, skipkeys=True))
main()
