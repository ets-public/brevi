# CPython (granted with --allow-py os) is held to the granted folders by the kernel: os.open inside them works,
# outside them (or through a link that leads out) fails as a PermissionError.
import os

def try_read(p):
    try:
        fd = os.open(p, os.O_RDONLY)
        data = os.read(fd, 100)
        os.close(fd)
        return "read " + data.decode().strip()
    except PermissionError:
        return "refused"
    except OSError as e:
        return "other error"

def try_write(p):
    try:
        fd = os.open(p, os.O_WRONLY | os.O_CREAT, 0o644)
        os.write(fd, "x".encode())
        os.close(fd)
        os.remove(p)
        return "wrote"
    except PermissionError:
        return "refused"

print(try_read("tests/sandbox/scoped/in/a.txt"))
print(try_read("tests/sandbox/scoped/secret.txt"))
print(try_read("tests/sandbox/scoped/in/link.txt"))
print(try_read("/etc/passwd"))
print(sorted(os.listdir("tests/sandbox/scoped/in")))
try:
    os.listdir("tests/sandbox/scoped")
    print("listed")
except PermissionError:
    print("list refused")
print(try_write("tests/sandbox/scoped/out/new.txt"))
print(try_write("tests/sandbox/scoped/in/new.txt"))
print(try_write("tests/sandbox/scoped/new.txt"))
