# Rates and caps in cents: (rate per started hour, daily cap)
RATES = {
    'car': (250, 2000),
    'moto': (120, 800),
    'van': (400, 3000),
}


def fee_cents(vtype, minutes):
    """Fee in cents for a stay of the given number of minutes."""
    if minutes <= 15:
        return 0
    rate, cap = RATES[vtype]
    days, rest = divmod(minutes, 1440)
    hours = -(-rest // 60)
    return days * cap + min(hours * rate, cap)


def fmt_money(cents):
    return f"{cents // 100}.{cents % 100:02d}"


def fmt_stay(minutes):
    return f"{minutes // 60}h{minutes % 60:02d}m"
