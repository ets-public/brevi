import sys
import re

INT = re.compile(r'-?\d+')
MISSING = object()

store = {}
logs = []  # one undo log per open transaction; each entry is (key, old value or MISSING)


def set_key(k, v):
    if logs:
        logs[-1].append((k, store.get(k, MISSING)))
    store[k] = v


def del_key(k):
    if k not in store:
        return False
    if logs:
        logs[-1].append((k, store[k]))
    del store[k]
    return True


def main():
    for line in sys.stdin.read().splitlines():
        parts = line.split()
        if not parts:
            continue
        cmd = parts[0]
        args = parts[1:]

        if cmd == 'SET' and len(args) == 2:
            set_key(args[0], args[1])
        elif cmd == 'GET' and len(args) == 1:
            v = store.get(args[0])
            print('NULL' if v is None else v)
        elif cmd == 'DEL' and len(args) == 1:
            if not del_key(args[0]):
                print('NOT FOUND')
        elif cmd == 'INCR' and len(args) == 2:
            k, n = args
            if not INT.fullmatch(n):
                print('ERR not an integer')
                continue
            cur = store.get(k)
            if cur is None:
                base = 0
            elif INT.fullmatch(cur):
                base = int(cur)
            else:
                print('ERR not an integer')
                continue
            new = base + int(n)
            set_key(k, str(new))
            print(new)
        elif cmd == 'COUNT' and len(args) == 1:
            print(sum(1 for v in store.values() if v == args[0]))
        elif cmd == 'KEYS' and len(args) == 1:
            ks = sorted(k for k in store if k.startswith(args[0]))
            print(' '.join(ks) if ks else '(empty)')
        elif cmd == 'BEGIN' and not args:
            logs.append([])
        elif cmd == 'ROLLBACK' and not args:
            if not logs:
                print('NO TRANSACTION')
            else:
                log = logs.pop()
                for k, old in reversed(log):
                    if old is MISSING:
                        store.pop(k, None)
                    else:
                        store[k] = old
        elif cmd == 'COMMIT' and not args:
            if not logs:
                print('NO TRANSACTION')
            else:
                logs.clear()
        else:
            print('ERR unknown command')


main()
