"""One way to print a share, used by every published conformance figure (tools/conformance.py's reports,
tools/release_notes.py's notes and its --check)."""


def pct(a, b):
    """a/b as a percentage to one decimal, rounded half up once from the counts. Rounding a stored share first
    rounds twice (945/1072 is 88.153%, stored as 0.8815, printed 88.1%), and format(x, ".1f") rounds the float
    half-even (1/16 printed 6.2%)."""
    a, b = int(a), int(b)
    if b <= 0:
        return "0.0%"
    tenths = (a * 2000 // b + 1) // 2
    return f"{tenths // 10}.{tenths % 10}%"
