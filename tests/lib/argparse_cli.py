# argparse builds natively (Brevi's copy of CPython 3.13's): parsing, errors, help text, subcommands.
import argparse


class Parser(argparse.ArgumentParser):
    def error(self, message):
        print("error:", message)
        raise SystemExit(2)


def run(p, argv):
    try:
        print(p.parse_args(argv))
    except SystemExit as e:
        print("exit", e.code)


def positive(s):
    v = int(s)
    if v <= 0:
        raise argparse.ArgumentTypeError(f"{s} is not positive")
    return v


p = Parser(prog="tool", description="Process some files. This description is long enough to wrap over more than one line of help.", epilog="See the docs.")
p.add_argument("files", nargs="*", help="input files")
p.add_argument("-v", "--verbose", action="count", default=0, help="more output")
p.add_argument("-o", "--output", metavar="PATH", help="where to write")
p.add_argument("--mode", choices=["fast", "slow"], default="fast", help="speed (default: %(default)s)")
p.add_argument("--tag", action="append", dest="tags")
p.add_argument("--limit", type=positive, default=10)
p.add_argument("--ratio", type=float)
p.add_argument("--dry-run", action="store_true")
p.add_argument("--color", action=argparse.BooleanOptionalAction, default=True)
p.add_argument("--pair", nargs=2, metavar=("A", "B"))
p.add_argument("--opt", nargs="?", const="C", default="D")
g = p.add_mutually_exclusive_group()
g.add_argument("--json", action="store_true")
g.add_argument("--csv", action="store_true")
adv = p.add_argument_group("advanced", "knobs for experts")
adv.add_argument("--threads", type=int, default=1)
print(p.format_help())
for argv in [[], ["a", "b", "-vvv"], ["-o", "x", "--mode", "slow", "--tag", "t1", "--tag", "t2"], ["--mode", "bogus"],
             ["--limit", "0"], ["--limit", "x"], ["--ratio", "2.5", "--no-color", "--dry"], ["--pair", "1"],
             ["--opt"], ["--json", "--csv"], ["--thr", "4"], ["--unknown"], ["--", "-x"], ["--limit=-3"]]:
    run(p, argv)

q = Parser(prog="git")
q.add_argument("--debug", action="store_true")
sub = q.add_subparsers(dest="cmd", required=True, help="sub-command help")
add = sub.add_parser("add", help="add a thing", aliases=["a"])
add.add_argument("name")
add.add_argument("-f", "--force", action="store_true")
rm = sub.add_parser("rm", help="remove things", formatter_class=argparse.ArgumentDefaultsHelpFormatter)
rm.add_argument("names", nargs="+")
rm.add_argument("-n", type=int, default=5, help="how many")
rm.set_defaults(func=len)
print(q.format_help())
print(rm.format_help())
for argv in [[], ["add", "x"], ["a", "x", "-f"], ["--debug", "rm", "p", "q"], ["rm"], ["nope"]]:
    run(q, argv)
args, rest = q.parse_known_args(["add", "x", "--extra", "y"])
print(args, rest)

r = argparse.ArgumentParser(prog="raw", formatter_class=argparse.RawDescriptionHelpFormatter, description="Lines:\n  one\n  two", usage="%(prog)s [options] N")
r.add_argument("n", type=int, help="a number with a very long help text that keeps going so that it must wrap onto several lines")
r.add_argument("-x", default=argparse.SUPPRESS)
r.add_argument("--hidden", help=argparse.SUPPRESS)
r.add_argument("--version", action="version", version="%(prog)s 1.2")
print(r.format_help())
print(r.format_usage(), end="")
print(r.parse_args(["5", "-x", "7"]), r.parse_args(["5"]))
ns = argparse.Namespace(a=1, b="x")
print(ns, ns.a, "a" in ns, ns == argparse.Namespace(a=1, b="x"), vars(ns))
e = argparse.ArgumentParser(exit_on_error=False)
e.add_argument("--k", type=int)
try:
    e.parse_args(["--k", "z"])
except argparse.ArgumentError as err:
    print("caught", err)
