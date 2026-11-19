# `m = months.setdefault(k, [0, 0])` (or `s = st[k]`), then `m[0] += v` while months is still read: the typed
# build writes m back into months right after each change, which the sharing proof now relies on (and checks).
def main():
    months = {}
    st = {}
    for d, k, v in [("2024-01-02", "a", 3), ("2024-01-09", "b", 4), ("2024-02-01", "a", 5)]:
        if v > 100:
            continue
        st.setdefault(k, [0, 0, 0])
        s = st[k]
        m = months.setdefault(d[:7], [0, 0])
        if k == "a":
            s[0] += v
            m[0] += v
        else:
            s[2] += v
            m[1] += v
    for mo in sorted(months):
        print(mo, months[mo][0], months[mo][1])
    print(st)


main()
