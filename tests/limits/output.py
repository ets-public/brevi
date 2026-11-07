# Prints forever: stopped once stdout and stderr together reach 1 KB, with what came before the limit kept.
i = 0
while True:
    print("line", i)
    i += 1
