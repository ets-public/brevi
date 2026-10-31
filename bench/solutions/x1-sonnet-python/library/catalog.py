import datetime

TIERS = {'basic': (2, 14, 25), 'plus': (5, 28, 10)}


class Book:
    def __init__(self, bid, copies, title):
        self.id = bid
        self.shelf = copies
        self.title = title
        self.waiting = []


class Member:
    def __init__(self, mid, tier, name):
        self.id = mid
        self.tier = tier
        self.name = name
        self.owes = 0  # cents
        self.loans = []  # [book_id, due_ordinal]

    @property
    def limit(self):
        return TIERS[self.tier][0]

    @property
    def days(self):
        return TIERS[self.tier][1]

    @property
    def daily(self):
        return TIERS[self.tier][2]


class Library:
    def __init__(self):
        self.books = {}
        self.members = {}
        self.holds = []  # [last_day_ordinal, book_id, member_id]


def to_ord(s):
    return datetime.date(int(s[:4]), int(s[5:7]), int(s[8:10])).toordinal()


def from_ord(o):
    return datetime.date.fromordinal(o).isoformat()


def money(c):
    s = "-" if c < 0 else ""
    c = abs(c)
    return "%s%d.%02d" % (s, c // 100, c % 100)
