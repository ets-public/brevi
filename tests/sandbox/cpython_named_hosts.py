# CPython cannot be held to the hosts named with --allow-net, so a program that would start it under them is
# refused, never given every host.
import socket
print(socket.gethostname() != "")
