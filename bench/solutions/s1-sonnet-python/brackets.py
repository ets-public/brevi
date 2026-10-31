import sys
pairs = {')': '(', ']': '[', '}': '{'}
for line in sys.stdin.read().splitlines():
    if line.endswith('\r'):
        line = line[:-1]
    st = []
    err = None
    for i, c in enumerate(line, 1):
        if c in '([{':
            st.append(c)
        elif c in pairs:
            if not st or st[-1] != pairs[c]:
                err = i
                break
            st.pop()
    if err is None and st:
        err = len(line) + 1
    print("ok" if err is None else "error at %d" % err)
