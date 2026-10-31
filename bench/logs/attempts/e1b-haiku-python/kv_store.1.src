import re
import sys

INT = re.compile(r'[+-]?[0-9]+')


def main():
    data = {}
    stack = []  # one undo frame per open transaction: key -> (existed, old value)
    out = []

    def record(key):
        if stack and key not in stack[-1]:
            stack[-1][key] = (key in data, data.get(key))

    for raw in sys.stdin.read().split('\n'):
        parts = raw.split()
        if not parts:
            continue
        cmd, args = parts[0], parts[1:]
        if cmd == 'SET' and len(args) == 2:
            record(args[0])
            data[args[0]] = args[1]
        elif cmd == 'GET' and len(args) == 1:
            out.append(data.get(args[0], 'NULL'))
        elif cmd == 'DEL' and len(args) == 1:
            if args[0] in data:
                record(args[0])
                del data[args[0]]
            else:
                out.append('NOT FOUND')
        elif cmd == 'INCR' and len(args) == 2:
            key, amount = args
            if not INT.fullmatch(amount):
                out.append('ERR not an integer')
                continue
            current = data.get(key, '0')
            if not INT.fullmatch(current):
                out.append('ERR not an integer')
                continue
            value = int(current) + int(amount)
            record(key)
            data[key] = str(value)
            out.append(str(value))
        elif cmd == 'COUNT' and len(args) == 1:
            out.append(str(sum(1 for v in data.values() if v == args[0])))
        elif cmd == 'KEYS' and len(args) <= 1:
            prefix = args[0] if args else ''
            keys = sorted(k for k in data if k.startswith(prefix))
            out.append(' '.join(keys) if keys else '(empty)')
        elif cmd == 'BEGIN' and not args:
            stack.append({})
        elif cmd == 'ROLLBACK' and not args:
            if not stack:
                out.append('NO TRANSACTION')
            else:
                frame = stack.pop()
                for key, (existed, old) in frame.items():
                    if existed:
                        data[key] = old
                    else:
                        data.pop(key, None)
        elif cmd == 'COMMIT' and not args:
            if not stack:
                out.append('NO TRANSACTION')
            else:
                stack.clear()
        else:
            out.append('ERR unknown command')

    if out:
        sys.stdout.write('\n'.join(out) + '\n')


main()
