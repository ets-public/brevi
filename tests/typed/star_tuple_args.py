NOT_FOUND = (404, "not found")
SIZES = (3, 4)


def resp(status, body):
    return f"{status} {body}"


def area(w, h):
    return w * h


def parse(sq):
    c = ord(sq[0]) - ord("a")
    r = int(sq[1:]) - 1
    return c, r


def ok(c, r):
    return 0 <= c < 8 and 0 <= r < 8


def main():
    a = parse("b3")
    print(resp(*NOT_FOUND), area(*SIZES), ok(*a), a)


for sq in ["a1", "h8", "z9"]:
    p = parse(sq)
    print(sq, ok(*p))
main()
