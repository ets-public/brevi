import datetime, fractions, decimal
dt = datetime.datetime(2020, 1, 1)
print(isinstance(dt, datetime.date), datetime.datetime.__mro__, issubclass(datetime.datetime, datetime.date))
print(type(datetime.date(2020, 1, 1)).__name__, datetime.date.__module__, datetime.date.__qualname__, datetime.date)
print(type(fractions.Fraction(1, 2)).__name__, type(decimal.Decimal(1)).__name__, decimal.Decimal.__module__)
print(isinstance(datetime.date(2020,1,1), datetime.datetime), dt == datetime.date(2020,1,1), datetime.timedelta.__name__)
try: dt < datetime.date(2020,1,1)
except TypeError as e: print('TE', e)
print(type(dt) is datetime.datetime, type(dt).__mro__[1] is datetime.date)
