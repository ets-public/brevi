"""Book and member records and the library state that holds them."""

import datetime
from collections import deque

TIERS = {
    "basic": {"limit": 2, "days": 14, "fee": 25},
    "plus": {"limit": 5, "days": 28, "fee": 10},
}

HOLD_DAYS = 3
MAX_FEE = 500  # cents


class Book:
    def __init__(self, bid, title, copies):
        self.id = bid
        self.title = title
        self.copies = copies
        self.shelf = copies          # copies on the shelf
        self.loans = {}              # member id -> due day (ordinal)
        self.waitlist = deque()      # member ids, first in line first


class Member:
    def __init__(self, mid, name, tier):
        self.id = mid
        self.name = name
        self.tier = tier
        self.loans = []              # book ids, in lend order
        self.owes = 0                # cents, may go negative (credit)


class Hold:
    def __init__(self, book_id, member_id, last_day):
        self.book_id = book_id
        self.member_id = member_id
        self.last_day = last_day     # ordinal of the last collectable day


class Library:
    def __init__(self):
        self.books = {}
        self.members = {}
        self.holds = []


def day_of(text):
    """Parse YYYY-MM-DD into an ordinal, or None if it is not a valid date."""
    parts = text.split("-")
    if len(parts) != 3 or any(not p.isdigit() for p in parts):
        return None
    if len(parts[0]) != 4 or len(parts[1]) != 2 or len(parts[2]) != 2:
        return None
    try:
        return datetime.date(int(parts[0]), int(parts[1]), int(parts[2])).toordinal()
    except ValueError:
        return None


def date_text(day):
    return datetime.date.fromordinal(day).isoformat()


def money(cents):
    sign = "-" if cents < 0 else ""
    c = abs(cents)
    return "%s%d.%02d" % (sign, c // 100, c % 100)
