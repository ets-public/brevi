import sys, re
text = sys.stdin.read().split('\n')
k = text.index('---') if '---' in text else len(text)
ini, qs = text[:k], text[k+1:]
secs = {}
cur = 'default'
for l in ini:
    s = l.strip()
    if not s or s[0] in ';#': continue
    if s.startswith('[') and s.endswith(']'):
        cur = s[1:-1].strip(); secs.setdefault(cur, {}); continue
    if '=' in s:
        a, b = s.split('=', 1)
        secs.setdefault(cur, {})[a.strip()] = b.strip()
class Cyc(Exception): pass
def find(sec, ref):
    if '.' in ref:
        a, b = ref.split('.', 1)
        if a in secs and b in secs[a]: return (a, b)
        return None
    if ref in secs.get(sec, {}): return (sec, ref)
    if ref in secs.get('default', {}): return ('default', ref)
    return None
def resolve(sec, key, stack):
    if (sec, key) in stack: raise Cyc()
    stack = stack | {(sec, key)}
    v = secs[sec][key]
    def rep(m):
        f = find(sec, m.group(1).strip())
        if f is None: return ''
        return resolve(f[0], f[1], stack)
    return re.sub(r'\$\{([^}]*)\}', rep, v)
def get(sec, key):
    try: return resolve(sec, key, frozenset())
    except Cyc: return '<cycle>'
out = []
for q in qs:
    q = q.strip()
    if not q: continue
    if q == 'sections':
        out.append(' '.join(sorted(s for s in secs if secs[s]))); continue
    sec, _, key = q.partition('.')
    if key == '*':
        if sec not in secs: out.append('<missing>'); continue
        for kk in sorted(secs[sec]): out.append('%s=%s' % (kk, get(sec, kk)))
    elif sec in secs and key in secs[sec]:
        out.append(get(sec, key))
    else:
        out.append('<missing>')
print('\n'.join(out))
