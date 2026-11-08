# --allow-read DIR / --allow-write DIR: files only inside the folders the caller granted. A symlink or .. that
# leads outside is refused, and so is a name outside that does not exist (nothing says whether it does).
for p in ["tests/sandbox/scoped/in/a.txt", "tests/sandbox/scoped/secret.txt", "tests/sandbox/scoped/in/link.txt",
          "tests/sandbox/scoped/in/../secret.txt", "tests/sandbox/scoped/in/missing.txt", "/etc/hostname", "/nowhere/x"]:
    try:
        with open(p) as f:
            print("read", p, f.read().strip())
    except Exception as e:
        print("refused", p, e)
for p in ["tests/sandbox/scoped/out/x.txt", "tests/sandbox/scoped/in/y.txt", "tests/sandbox/scoped/out/../z.txt"]:
    try:
        with open(p, "w") as f:
            f.write("w")
        print("wrote", p)
    except Exception as e:
        print("refused", p, e)
