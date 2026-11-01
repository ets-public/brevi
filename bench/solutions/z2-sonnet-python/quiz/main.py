import sys
from key import parse_key_line, grade_answer
from grade import Gradebook

msgs = []
questions = {}
order = []
student_lines = []
state = 0
n = 0
for raw in sys.stdin.read().split('\n'):
    n += 1
    s = raw.strip()
    if not s:
        continue
    if state == 0:
        if s == 'KEY':
            state = 1
        continue
    if state == 1:
        if s == 'KEY':
            continue
        if s == 'ANSWERS':
            state = 2
            continue
        r = parse_key_line(s, questions)
        if r is None:
            msgs.append('key line %d: bad question' % n)
        else:
            questions[r[0]] = r[1]
            order.append(r[0])
        continue
    if s in ('KEY', 'ANSWERS'):
        continue
    student_lines.append((n, s))

maxpts = sum(q['points'] for q in questions.values())
book = Gradebook(order, maxpts)
for n, s in student_lines:
    w = s.split()
    name = w[0]
    if book.has(name):
        msgs.append('line %d: duplicate student' % n)
        continue
    per = {}
    for tok in w[1:]:
        if '=' not in tok or tok.endswith('=') :
            msgs.append('line %d: bad answer %s' % (n, tok))
            continue
        qid, ans = tok.split('=', 1)
        if qid not in questions:
            msgs.append('line %d: unknown question %s' % (n, qid))
            continue
        if qid in per:
            msgs.append('line %d: answered twice %s' % (n, qid))
            continue
        per[qid] = grade_answer(questions[qid], ans)
    book.add(name, per)

for m in msgs:
    print(m)
if not order:
    print('empty key')
elif not book.names:
    print('no students')
else:
    for x in book.names:
        print('%s %d/%d %d%%' % (x, book.total(x), maxpts, book.pct(x)))
    for q in order:
        print('%s avg %s' % (q, book.avg(q)))
    names, best = book.top()
    print('top: %s (%d)' % (', '.join(names), best))
