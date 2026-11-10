# CPython's timedelta constructor raises OverflowError once the normalized day count is out of range
# (|days| must be <= 999999999), and the C int message when the count cannot even fit the stored field.
from datetime import timedelta

try:
    print(timedelta(days=10**9))
except OverflowError as e:
    print("OverflowError", e)

for kw in [dict(days=999999999), dict(days=-999999999), dict(days=-10**9),
           dict(days=999999999, seconds=86400), dict(weeks=142857143),
           dict(seconds=86400), dict(days=2**31)]:
    try:
        print(timedelta(**kw))
    except OverflowError as e:
        print("OverflowError", e)
