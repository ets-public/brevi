import sys
prog = []; labels = {}
def wrap(x):
    x &= (1 << 64) - 1
    return x - (1 << 64) if x >> 63 else x
for no, l in enumerate(sys.stdin.read().split('\n'), 1):
    s = l.split('#', 1)[0].strip()
    if not s: continue
    if s.endswith(':') and ' ' not in s:
        labels[s[:-1]] = len(prog); continue
    p = s.split()
    prog.append((no, p[0], p[1:]))
NOARG = {'pop','dup','swap','add','sub','mul','div','mod','ret','print','halt'}
JMP = {'jmp','jz','jnz','call'}
for no, op, a in prog:
    if op not in NOARG and op not in JMP and op != 'push':
        print("error line %d: unknown instruction %s" % (no, op)); sys.exit()
for no, op, a in prog:
    if op in JMP and (not a or a[0] not in labels):
        print("error line %d: unknown label %s" % (no, a[0] if a else '')); sys.exit()
out = []
def fin():
    if out: print('\n'.join(out))
st = []; cs = []; pc = 0; steps = 0
while pc < len(prog):
    if steps >= 10000:
        out.append("error: step limit"); break
    steps += 1
    no, op, a = prog[pc]; pc += 1
    need = {'pop':1,'dup':1,'swap':2,'add':2,'sub':2,'mul':2,'div':2,'mod':2,'jz':1,'jnz':1,'print':1}.get(op, 0)
    if len(st) < need:
        out.append("error line %d: stack underflow" % no); break
    if op == 'push': st.append(wrap(int(a[0])))
    elif op == 'pop': st.pop()
    elif op == 'dup': st.append(st[-1])
    elif op == 'swap': st[-1], st[-2] = st[-2], st[-1]
    elif op in ('add','sub','mul','div','mod'):
        y = st.pop(); x = st.pop()
        if op == 'add': r = x + y
        elif op == 'sub': r = x - y
        elif op == 'mul': r = x * y
        else:
            if y == 0:
                out.append("error line %d: division by zero" % no); break
            q = abs(x) // abs(y)
            if (x < 0) != (y < 0): q = -q
            r = q if op == 'div' else x - q * y
        st.append(wrap(r))
    elif op == 'jmp': pc = labels[a[0]]
    elif op == 'jz':
        if st.pop() == 0: pc = labels[a[0]]
    elif op == 'jnz':
        if st.pop() != 0: pc = labels[a[0]]
    elif op == 'call': cs.append(pc); pc = labels[a[0]]
    elif op == 'ret':
        if not cs: break
        pc = cs.pop()
    elif op == 'print': out.append(str(st.pop()))
    elif op == 'halt': break
fin()
