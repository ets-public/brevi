# rates and daily caps in cents
RATE = {'car': 250, 'moto': 120, 'van': 400}
CAP = {'car': 2000, 'moto': 800, 'van': 3000}


def fee_cents(vtype, minutes):
    if minutes <= 15:
        return 0
    days = minutes // (24 * 60)
    rest = minutes % (24 * 60)
    hours = -(-rest // 60)
    rest_fee = min(hours * RATE[vtype], CAP[vtype])
    return days * CAP[vtype] + rest_fee


def money(cents):
    return '%d.%02d' % (cents // 100, cents % 100)


def stay_text(minutes):
    return '%dh%02dm' % (minutes // 60, minutes % 60)
