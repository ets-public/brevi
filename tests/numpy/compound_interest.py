import numpy as np

# Fixed inputs
principal = 10_000.0          # initial investment
annual_rate = 0.05            # 5% annual interest
compounds_per_year = 12       # monthly compounding
years = np.arange(0, 11)      # 0 to 10 years

# Compound interest formula: A = P * (1 + r/n) ** (n * t)
balances = principal * (1 + annual_rate / compounds_per_year) ** (compounds_per_year * years)
interest_earned = balances - principal

# Year-over-year growth
yearly_gain = np.diff(balances, prepend=balances[0])

# Print results
print(f"Principal: ${principal:,.2f} | Rate: {annual_rate:.2%} | Compounding: {compounds_per_year}x/year\n")
print(f"{'Year':>4} {'Balance':>14} {'Total Interest':>16} {'Yearly Gain':>14}")
print("-" * 52)
for y, b, i, g in zip(years, balances, interest_earned, yearly_gain):
    print(f"{y:>4} {b:>14,.2f} {i:>16,.2f} {g:>14,.2f}")

# Compare different rates using broadcasting
rates = np.array([0.03, 0.05, 0.07])
table = principal * (1 + rates[:, None] / compounds_per_year) ** (compounds_per_year * years[None, :])

print("\nBalance after 10 years at different rates:")
for r, final in zip(rates, table[:, -1]):
    print(f"  {r:.0%}: ${final:,.2f}")
