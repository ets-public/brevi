# warnings.warn(..., stacklevel=2) names the caller's line, which the dynamic build does not keep: it refuses
# to build this, so CPython runs it.
import warnings


def old():
    warnings.warn("old() is deprecated", DeprecationWarning, stacklevel=2)


old()
