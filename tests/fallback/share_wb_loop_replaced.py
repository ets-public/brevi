# The item is replaced after the first change, inside a loop: from the second time round m is no longer in q.
def main():
    q = {"a": [1]}
    m = q["a"]
    for i in range(3):
        m.append(i)
        q["a"] = [9]
    print(q, m)


main()
