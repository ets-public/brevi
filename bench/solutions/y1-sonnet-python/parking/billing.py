RATES = {'car': (250, 2000), 'moto': (120, 800), 'van': (400, 3000)}


def stay_text(d):
    return '%dh%02dm' % (d // 60, d % 60)


def fee_cents(kind, d):
    if d <= 15:
        return 0
    rate, cap = RATES[kind]
    days, rest = divmod(d, 1440)
    hours = -(-rest // 60)
    return days * cap + min(hours * rate, cap)


def money(c):
    return '%d.%02d' % (c // 100, c % 100)
