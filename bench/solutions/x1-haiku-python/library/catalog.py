TIERS = {
    'basic': {'limit': 2, 'days': 14, 'rate': 25},   # rate in cents per day late
    'plus': {'limit': 5, 'days': 28, 'rate': 10},
}


class Hold:
    def __init__(self, member_id, last):
        self.member_id = member_id
        self.last = last  # last day (date) the copy can be collected


class Book:
    def __init__(self, bid, title, copies):
        self.id = bid
        self.title = title
        self.copies = copies
        self.shelf = copies      # copies on the shelf (not on loan, not held)
        self.waitlist = []       # member IDs, first in line first
        self.holds = []          # Hold objects, one per held copy


class Member:
    def __init__(self, mid, name, tier):
        self.id = mid
        self.name = name
        self.tier = tier
        self.owes = 0            # cents; may be negative (credit)
        self.loans = []          # (book_id, due date), in lending order


class Library:
    def __init__(self):
        self.books = {}
        self.members = {}
