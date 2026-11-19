# m was months["a"], but that item is replaced before m changes: writing m back would undo the replacement.
def main():
    months = {"a": [1], "b": [2]}
    k = "a"
    m = months[k]
    months[k] = [5]
    m.append(1)
    print(months, m)


main()
