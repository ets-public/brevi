import re
import sys

ABSENT = object()
INT_RE = re.compile(r'^[-+]?\d+$')


def main():
    store = {}
    # Each open transaction holds an undo log: key -> previous value (or ABSENT).
    txns = []

    def record(key):
        if txns:
            log = txns[-1]
            if key not in log:
                log[key] = store.get(key, ABSENT)

    def set_key(key, value):
        record(key)
        store[key] = value

    def del_key(key):
        record(key)
        del store[key]

    def parse_int(s):
        if INT_RE.match(s):
            return int(s)
        return None

    out = []
    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        parts = line.split()
        cmd = parts[0]
        args = parts[1:]

        if cmd == 'SET' and len(args) == 2:
            set_key(args[0], args[1])
        elif cmd == 'GET' and len(args) == 1:
            out.append(store.get(args[0], 'NULL'))
        elif cmd == 'DEL' and len(args) == 1:
            if args[0] in store:
                del_key(args[0])
            else:
                out.append('NOT FOUND')
        elif cmd == 'INCR' and len(args) == 2:
            key, delta_s = args
            delta = parse_int(delta_s)
            if delta is None:
                out.append('ERR not an integer')
                continue
            if key in store:
                cur = parse_int(store[key])
                if cur is None:
                    out.append('ERR not an integer')
                    continue
            else:
                cur = 0
            new_val = cur + delta
            set_key(key, str(new_val))
            out.append(str(new_val))
        elif cmd == 'COUNT' and len(args) == 1:
            n = sum(1 for v in store.values() if v == args[0])
            out.append(str(n))
        elif cmd == 'KEYS' and len(args) == 1:
            p = args[0]
            ks = sorted(k for k in store if k.startswith(p))
            out.append(' '.join(ks) if ks else '(empty)')
        elif cmd == 'BEGIN' and len(args) == 0:
            txns.append({})
        elif cmd == 'ROLLBACK' and len(args) == 0:
            if not txns:
                out.append('NO TRANSACTION')
            else:
                log = txns.pop()
                for key, old in log.items():
                    if old is ABSENT:
                        store.pop(key, None)
                    else:
                        store[key] = old
        elif cmd == 'COMMIT' and len(args) == 0:
            if not txns:
                out.append('NO TRANSACTION')
            else:
                # Changes already applied to the store; just discard undo logs.
                # Changes made in an inner transaction must still be undoable by
                # an enclosing transaction, so merge the logs downward.
                txns.clear()
        else:
            out.append('ERR unknown command')

    if out:
        sys.stdout.write('\n'.join(out) + '\n')


if __name__ == '__main__':
    main()
