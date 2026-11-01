import sys
from key import parse_key_line, grade_answer
from grade import Grades

lines = sys.stdin.read().split('\n')
state = 0
questions = {}
order = []
students = []
msgs = []
g = None
for n, raw in enumerate(lines, 1):
    s = raw.strip()
    if not s:
        continue
    if state == 0:
        if s == 'KEY':
            state = 1
        continue
    if state == 1:
        if s == 'ANSWERS':
            state = 2
            continue
        if s == 'KEY':
            continue
        q = parse_key_line(s)
        if q is None or q[0] in questions:
            msgs.append('key line %d: bad question' % n)
        else:
            questions[q[0]] = q
            order.append(q[0])
        continue
    if s in ('KEY', 'ANSWERS'):
        continue
    if g is None:
        g = Grades(order, sum(questions[i][2] for i in order))
    w = s.split()
    name = w[0]
    if g.has(name):
        msgs.append('line %d: duplicate student' % n)
        continue
    given = {}
    scores = {}
    for tok in w[1:]:
        if '=' not in tok or tok.split('=', 1)[1] == '':
            msgs.append('line %d: bad answer %s' % (n, tok))
            continue
        qid, ans = tok.split('=', 1)
        if qid not in questions:
            msgs.append('line %d: unknown question %s' % (n, qid))
            continue
        if qid in given:
            msgs.append('line %d: answered twice %s' % (n, qid))
            continue
        given[qid] = ans
        scores[qid] = grade_answer(questions[qid], ans)
    g.add(name, scores)

out = list(msgs)
if not order:
    out.append('empty key')
elif g is None or not g.students:
    out.append('no students')
else:
    for name, sc in g.students:
        out.append('%s %d/%d %d%%' % (name, g.total(sc), g.maxpts, g.pct(sc)))
    for qid in order:
        out.append('%s avg %s' % (qid, g.avg(qid)))
    names, best = g.top()
    out.append('top: %s (%d)' % (', '.join(names), best))
print('\n'.join(out))
