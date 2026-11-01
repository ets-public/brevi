import sys

from key import parse_key
from grade import Gradebook


def main():
    raw_lines = sys.stdin.read().split("\n")

    state = "before"  # before -> key -> answers
    key_lines = []
    student_lines = []

    for idx, raw in enumerate(raw_lines):
        lineno = idx + 1
        text = raw.rstrip("\r")
        parts = text.split()
        if not parts:
            continue
        if state == "before":
            if parts == ["KEY"]:
                state = "key"
            continue
        if state == "key":
            if parts == ["ANSWERS"]:
                state = "answers"
                continue
            if parts == ["KEY"]:
                continue
            key_lines.append((lineno, text))
            continue
        # answers section
        if parts == ["ANSWERS"]:
            continue
        student_lines.append((lineno, text))

    questions, messages = parse_key(key_lines)
    question_ids = set(q["id"] for q in questions)
    book = Gradebook(questions)
    names_used = set()

    for lineno, text in student_lines:
        parts = text.split()
        name = parts[0]
        if name in names_used:
            messages.append("line %d: duplicate student" % lineno)
            continue
        names_used.add(name)
        answers = {}
        for tok in parts[1:]:
            if "=" not in tok or tok.split("=", 1)[1] == "":
                messages.append("line %d: bad answer %s" % (lineno, tok))
                continue
            qid, ans = tok.split("=", 1)
            if qid not in question_ids:
                messages.append("line %d: unknown question %s" % (lineno, qid))
                continue
            if qid in answers:
                messages.append("line %d: answered twice %s" % (lineno, qid))
                continue
            answers[qid] = ans
        book.add(name, answers)

    for m in messages:
        print(m)

    if not book.students:
        print("no students")
        return
    if not questions:
        print("empty key")
        return

    for name, score, pct in book.results():
        print("%s %d/%d %d%%" % (name, score, book.max_total, pct))
    for q in questions:
        print("%s avg %s" % (q["id"], book.average(q["id"])))
    best, names = book.top()
    print("top: %s (%d)" % (", ".join(names), best))


if __name__ == "__main__":
    main()
