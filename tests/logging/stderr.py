# What Brevi's logging writes to stderr: the last-resort handler (no handlers configured), the module-level
# functions' basicConfig(), a handler's "--- Logging error ---" report and logger.exception's traceback.
import logging
import sys
log = logging.getLogger(__name__)
log.warning("no handlers %s", 1)
log.info("hidden")
print("between", flush=True)
logging.warning("root warn %d", 2)
logging.info("root info hidden")
log.error("bad args %d", "x")
try:
    {}["k"]
except KeyError:
    log.exception("lookup failed")
try:
    raise ValueError("v")
except ValueError as e:
    log.error("with instance", exc_info=e)
logging.root.handlers[0].setFormatter(logging.Formatter("%(asctime)s|%(message)s", datefmt="[%%]"))
log.critical("timed")
sys.stdout.flush()
try:
    try:
        int("q")
    except ValueError as e:
        raise RuntimeError("wrapped") from e
except RuntimeError:
    log.exception("chained")
try:
    try:
        [][1]
    except IndexError:
        raise KeyError("during")
except KeyError:
    log.exception("context")
