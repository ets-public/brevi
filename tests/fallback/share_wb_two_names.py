# Two names for one item of a dict filled by setdefault: a change through one shows through the other in Python
# (the sharing proof read the dict's items as never lists, so the typed build printed [0, 0] for x).
def main():
    months = {}
    for d, v in [("2024-01-02", 3), ("2024-01-09", 4)]:
        months.setdefault(d[:7], [0, 0])
    x = months["2024-01"]
    y = months.setdefault("2024-01", [])
    y.append(9)
    print(x, months)


main()
