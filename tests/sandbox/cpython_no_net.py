# CPython (granted with --allow-py socket) gets no network without a network grant: the kernel (seccomp) makes
# socket() fail, for every family, including a Unix socket by path (a Docker or database daemon). socketpair()
# still works: it reaches nothing outside the program.
import socket

def try_socket(family):
    try:
        s = socket.socket(family, socket.SOCK_STREAM)
        s.close()
        return "made"
    except PermissionError:
        return "refused"

print(try_socket(socket.AF_INET))
print(try_socket(socket.AF_INET6))
print(try_socket(socket.AF_UNIX))
a, b = socket.socketpair()
a.sendall("ping".encode())
print(b.recv(4).decode())
a.close()
b.close()
