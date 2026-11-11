# sys.flags: a built program runs as a plain `python3 prog.py`, so the command-line options read 0/False.
import sys

f = sys.flags
print(f.debug, f.inspect, f.interactive, f.optimize, f.dont_write_bytecode, f.verbose, f.quiet)
print(f.no_user_site, f.no_site, f.ignore_environment, f.bytes_warning, f.isolated, f.warn_default_encoding)
print(f.dev_mode, f.safe_path, type(f).__name__)
if sys.flags.optimize < 2:
    print("docstrings kept")
