class Book:
    def __init__(self, bid, copies, title):
        self.id = bid
        self.title = title
        self.shelf = copies
        self.waiting = []


class Member:
    def __init__(self, mid, tier, name):
        self.id = mid
        self.tier = tier
        self.name = name
        self.owes = 0  # cents
        self.loans = []  # list of [book_id, due_date]


class Library:
    def __init__(self):
        self.books = {}
        self.members = {}
        self.holds = []  # [last_day, book_id, member_id]
