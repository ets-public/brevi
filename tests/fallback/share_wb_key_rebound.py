# The key name is bound again between `m = q[k]` and the change: a write-back would go to the wrong item.
def main():
    q = {"a": [1], "b": [2]}
    k = "a"
    m = q[k]
    k = "b"
    m.append(1)
    print(q, m)


main()
