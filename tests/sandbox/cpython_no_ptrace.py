# CPython (granted with --allow-py) under --sandbox reaches no other process: the kernel (seccomp) makes ptrace,
# process_vm_readv, process_vm_writev, pidfd_open, pidfd_getfd and userfaultfd fail with EPERM. Each call is
# made so that, unheld, it would answer something else (0, a file descriptor, ESRCH, EBADF or EINVAL).
import ctypes
import os

libc = ctypes.CDLL(None, use_errno=True)
libc.syscall.restype = ctypes.c_long
arm = os.uname().machine == "aarch64"

def call(name, nr, a, b, c):
    L = ctypes.c_long
    r = libc.syscall(L(nr), L(a), L(b), L(c), L(0), L(0), L(0))
    if r >= 0:
        return name + " ran"
    if ctypes.get_errno() == 1:
        return name + " refused"
    return name + " failed otherwise"

me = int(os.getpid())
# PTRACE_PEEKDATA of no process: ESRCH unheld
print(call("ptrace", 117 if arm else 101, 2, 0x3FFFFFFF, 0))
# no ranges: 0 unheld
print(call("process_vm_readv", 270 if arm else 310, me, 0, 0))
print(call("process_vm_writev", 271 if arm else 311, me, 0, 0))
# no pidfd: EBADF unheld
print(call("pidfd_getfd", 438, -1, 0, 0))
# UFFD_USER_MODE_ONLY and a bad flag: EINVAL unheld
print(call("userfaultfd", 282 if arm else 323, 1 | 0x40000000, 0, 0))
try:
    os.close(os.pidfd_open(me))
    print("pidfd_open ran")
except PermissionError:
    print("pidfd_open refused")
