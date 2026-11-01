import sys
import key
import grade


def main():
    questions = {}
    g = grade.Grade(questions)
    messages = []
    section = 'before'

    for n, raw in enumerate(sys.stdin, 1):
        line = raw.strip()
        if not line:
            continue
        if section == 'before':
            if line == 'KEY':
                section = 'key'
            continue
        if section == 'key':
            if line == 'ANSWERS':
                section = 'students'
            elif line == 'KEY':
                pass
            elif not key.add_question(questions, line.split()):
                messages.append('key line %d: bad question' % n)
            continue

        # student section
        if line in ('ANSWERS', 'KEY'):
            continue
        tokens = line.split()
        name = tokens[0]
        if g.has_name(name):
            messages.append('line %d: duplicate student' % n)
            continue
        answers = {}
        for tok in tokens[1:]:
            if '=' not in tok or tok.split('=', 1)[1] == '':
                messages.append('line %d: bad answer %s' % (n, tok))
                continue
            qid, ans = tok.split('=', 1)
            if qid not in questions:
                messages.append('line %d: unknown question %s' % (n, qid))
                continue
            if qid in answers:
                messages.append('line %d: answered twice %s' % (n, qid))
                continue
            answers[qid] = ans
        g.add_student(name, answers)

    out = list(messages)
    if not g.students:
        out.append('no students')
    elif not questions:
        out.append('empty key')
    else:
        out.extend(g.student_lines())
        out.extend(g.average_lines())
        out.append(g.top_line())
    print('\n'.join(out))


if __name__ == '__main__':
    main()
