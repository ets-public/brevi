# A module with top-level statements is not joined by the typed build: the whole program builds over dynamic
# values, which run the module once on import as CPython does.
import _setup
import _setup as again

again.LOG.append(10)
print(_setup.total(), _setup.LOG)
