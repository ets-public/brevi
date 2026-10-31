import sys
import re

MISSING = object()
INT = re.compile(r'-?[0-9]+')

data = {}
frames = []  # one dict per open BEGIN: key -> value before first change in that transaction


def record(key):
    if frames and key not in frames[-1]:
        frames[-1][key] = data.get(key, MISSING)


def main():
    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        parts = line.split()
        cmd = parts[0]
        args = parts[1:]

        if cmd == 'SET' and len(args) == 2:
            record(args[0])
            data[args[0]] = args[1]
        elif cmd == 'GET' and len(args) == 1:
            print(data.get(args[0], 'NULL'))
        elif cmd == 'DEL' and len(args) == 1:
            k = args[0]
            if k in data:
                record(k)
                del data[k]
            else:
                print('NOT FOUND')
        elif cmd == 'INCR' and len(args) == 2:
            k, n = args
            if not INT.fullmatch(n):
                print('ERR not an integer')
                continue
            cur = data.get(k)
            if cur is None:
                cur = '0'
            if not INT.fullmatch(cur):
                print('ERR not an integer')
                continue
            new = int(cur) + int(n)
            record(k)
            data[k] = str(new)
            print(new)
        elif cmd == 'COUNT' and len(args) == 1:
            print(sum(1 for v in data.values() if v == args[0]))
        elif cmd == 'KEYS' and len(args) <= 1:
            p = args[0] if args else ''
            ks = sorted(k for k in data if k.startswith(p))
            print(' '.join(ks) if ks else '(empty)')
        elif cmd == 'BEGIN' and not args:
            frames.append({})
        elif cmd == 'ROLLBACK' and not args:
            if not frames:
                print('NO TRANSACTION')
            else:
                f = frames.pop()
                for k, old in f.items():
                    if old is MISSING:
                        data.pop(k, None)
                    else:
                        data[k] = old
        elif cmd == 'COMMIT' and not args:
            if not frames:
                print('NO TRANSACTION')
            else:
                frames.clear()
        else:
            print('ERR unknown command')


if __name__ == '__main__':
    main()
