# Brevi's own logging (src/dynpy/py/logging.py): loggers, levels, handlers, the three format styles,
# filters, LoggerAdapter and exc_info, written to stdout and a StringIO.
import logging, sys, io
log = logging.getLogger(__name__)
print(logging.getLevelName(20), logging.getLevelName("ERROR"), logging.getLevelName(25))
logging.basicConfig(level=logging.DEBUG, format="%(levelname)-8s %(name)s | %(message)s", stream=sys.stdout)
log.debug("debug %d %s", 3, "x")
a = logging.getLogger("a.b.c")
b = logging.getLogger("a")
print(a.parent.name, b.parent.name, a.getEffectiveLevel())
b.setLevel("ERROR")
a.warning("dropped")
a.error("kept %(k)s", {"k": "v"})
buf = io.StringIO()
h = logging.StreamHandler(buf)
h.setFormatter(logging.Formatter("{levelname}:{message}", style="{"))
b.addHandler(h)
b.propagate = False
a.critical("to buf")
print(repr(buf.getvalue()))
try:
    1 / 0
except ZeroDivisionError:
    a.exception("boom")
print(buf.getvalue().splitlines()[-1])
class F(logging.Filter):
    def filter(self, r):
        r.extra_field = "E"
        return "secret" not in r.getMessage()
h2 = logging.StreamHandler(sys.stdout)
h2.setFormatter(logging.Formatter("$levelname [$extra_field] ${message}", style="$"))
h2.addFilter(F())
c = logging.getLogger("c"); c.addHandler(h2); c.propagate = False
c.warning("public"); c.warning("secret")
ad = logging.LoggerAdapter(c, {"user": "u1"})
c.addFilter(lambda r: setattr(r, "extra_field", getattr(r, "user", "-")) or True)
ad.warning("adapter")
c.warning("x", extra={"user": "u2"})
print(repr(log), repr(logging.getLogger()), repr(h2)[:16])
logging.disable(logging.CRITICAL)
c.critical("disabled")
logging.disable(logging.NOTSET)
logging.getLogger("a.b").info("x")
print(logging.getLogger("a.b").parent.name, a.parent.name)
try:
    logging.Formatter("%(levelname)", validate=True)
except ValueError as e:
    print("VE", e)
rec = logging.makeLogRecord({"msg": "hello %s", "args": ("w",), "levelname": "INFO", "name": "n"})
print(logging.Formatter().format(rec))
print(logging.Formatter("%(asctime)s", datefmt="%Y").format(rec) == logging.Formatter("%(asctime)s", datefmt="%Y").format(rec))
