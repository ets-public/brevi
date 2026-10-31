TIERS = {
    'basic': {'max': 2, 'days': 14, 'fee': 25},
    'plus': {'max': 5, 'days': 28, 'fee': 10},
}


class Book:
    def __init__(self, bid, copies, title):
        self.id = bid
        self.title = title
        self.shelf = copies
        self.waiting = []  # member ids
        self.holds = []    # [member id, last day (date ordinal)]


class Member:
    def __init__(self, mid, tier, name):
        self.id = mid
        self.tier = tier
        self.name = name
        self.owes = 0  # cents
        self.loans = []  # [book id, due date]


class Library:
    def __init__(self):
        self.books = {}
        self.members = {}


def cents(s):
    if '.' in s:
        a, b = s.split('.', 1)
        b = (b + '00')[:2]
    else:
        a, b = s, '00'
    return int(a or '0') * 100 + int(b)


def fmt(c):
    sign = '-' if c < 0 else ''
    c = abs(c)
    return "%s%d.%02d" % (sign, c // 100, c % 100)
