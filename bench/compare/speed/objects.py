class Account:
    def __init__(self, name, balance):
        self.name = name
        self.balance = balance
        self.history = []

    def deposit(self, amount):
        self.balance += amount
        self.history.append(amount)

    def withdraw(self, amount):
        if amount > self.balance:
            return False
        self.balance -= amount
        self.history.append(-amount)
        return True

n = int(input())
accounts = [Account(f"acct{i}", (i * 37) % 500) for i in range(1000)]
fails = 0
seed = 12345
for step in range(n):
    seed = (seed * 1103515245 + 12345) % 2147483648
    a = accounts[seed % 1000]
    amt = (seed >> 8) % 300
    if seed % 3 == 0:
        if not a.withdraw(amt):
            fails += 1
    else:
        a.deposit(amt)
top = sorted(accounts, key=lambda a: (-a.balance, a.name))[:3]
for a in top:
    print(a.name, a.balance, len(a.history))
print("fails", fails, "moves", sum(len(a.history) for a in accounts))
