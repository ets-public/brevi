# Opens 50 files at once where the caller allows 16 open: the open past the limit fails with OSError.
import os

fds = []
try:
    for i in range(50):
        fds.append(os.open("/dev/null", os.O_RDONLY))
    print("opened", len(fds))
except OSError as e:
    print("refused after", len(fds), "opens")
