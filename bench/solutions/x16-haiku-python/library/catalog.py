"""Book and member records, and the library state that holds them."""

TIERS = {
    'basic': {'limit': 2, 'days': 14, 'fee': 25},   # fee in cents per day
    'plus': {'limit': 5, 'days': 28, 'fee': 10},
}
MAX_FEE = 500        # cents per loan
HOLD_DAYS = 3


class Book:
    def __init__(self, bid, title, copies):
        self.id = bid
        self.title = title
        self.shelf = copies      # copies on the shelf, free to lend
        self.waiting = []        # member IDs, first in line first


class Member:
    def __init__(self, mid, name, tier):
        self.id = mid
        self.name = name
        self.tier = tier
        self.owed = 0            # cents, may be negative (credit)
        self.loans = []          # Loan objects in the order lent


class Loan:
    def __init__(self, book_id, due):
        self.book_id = book_id
        self.due = due           # datetime.date


class Hold:
    def __init__(self, member_id, book_id, last):
        self.member_id = member_id
        self.book_id = book_id
        self.last = last         # last day the copy can be collected


class Library:
    def __init__(self):
        self.books = {}
        self.members = {}
        self.holds = []


def money(c):
    sign = '-' if c < 0 else ''
    c = abs(c)
    return '%s%d.%02d' % (sign, c // 100, c % 100)
