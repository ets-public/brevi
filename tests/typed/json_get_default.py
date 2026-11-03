# j.get(k), j.get(k, []) and j.get(k, {}) on JSON values.
import json
data = json.loads('{"customers": {"1": {"n": "a"}}, "orders": [{"id": "A", "customer": 1, "lines": [1, 2]}, {"customer": 2}]}')
def main():
    customers = data.get('customers', {})
    for order in data.get('orders', []):
        oid = order.get('id')
        cid = order.get('customer')
        lines = order.get('lines', [])
        cust = customers.get(str(cid))
        if cust is None:
            print("unknown", cid, oid, len(lines))
            continue
        print(cust["n"], oid, len(lines))
main()
