import datetime

TIERS = {
    'basic': {'limit': 2, 'days': 14, 'fee': 25},
    'plus': {'limit': 5, 'days': 28, 'fee': 10},
}
MAX_FEE = 500
HOLD_DAYS = 3


class Hold:
    def __init__(self, member_id, last):
        self.member_id = member_id
        self.last = last


class Book:
    def __init__(self, bid, title, copies):
        self.id = bid
        self.title = title
        self.copies = copies
        self.shelf = copies
        self.waitlist = []
        self.holds = []


class Member:
    def __init__(self, mid, name, tier):
        self.id = mid
        self.name = name
        self.tier = tier
        self.owes = 0
        self.loans = []


class Library:
    def __init__(self):
        self.books = {}
        self.members = {}


def parse_date(s):
    return datetime.date(int(s[0:4]), int(s[5:7]), int(s[8:10]))


def fmt_date(d):
    return "%04d-%02d-%02d" % (d.year, d.month, d.day)


def to_cents(s):
    if '.' in s:
        a, b = s.split('.')
    else:
        a, b = s, ''
    return int(a or '0') * 100 + int((b + '00')[:2])


def money(c):
    sign = '-' if c < 0 else ''
    c = abs(c)
    return '%s%d.%02d' % (sign, c // 100, c % 100)
