import sys
pairs = {')': '(', ']': '[', '}': '{'}
data = sys.stdin.read().split('\n')
if data and data[-1] == '':
    data.pop()
for line in data:
    line = line.rstrip('\r')
    st = []
    err = None
    for i, ch in enumerate(line, 1):
        if ch in '([{':
            st.append(ch)
        elif ch in pairs:
            if not st or st[-1] != pairs[ch]:
                err = i
                break
            st.pop()
    if err is None and st:
        err = len(line) + 1
    print("ok" if err is None else "error at %d" % err)
