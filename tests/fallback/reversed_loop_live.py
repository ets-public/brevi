# #459: reversed() reads the list as the loop goes, so a write the loop makes is seen
a = [1, 2, 3]
for x in reversed(a):
    a[0] = 9
    print(x)
