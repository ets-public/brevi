import datetime

RATES = {'car': (250, 2000), 'moto': (120, 800), 'van': (400, 3000)}

def stay_minutes(t_in, t_out):
    return int((t_out - t_in).total_seconds() // 60)

def fee_cents(kind, minutes):
    rate, cap = RATES[kind]
    if minutes <= 15:
        return 0
    days, rest = divmod(minutes, 1440)
    hours = -(-rest // 60)
    return days * cap + min(hours * rate, cap)

def fmt_stay(minutes):
    return '%dh%02dm' % (minutes // 60, minutes % 60)

def money(c):
    return '%d.%02d' % (c // 100, c % 100)
