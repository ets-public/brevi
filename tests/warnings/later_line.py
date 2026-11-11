# warnings.warn from a statement whose call opens on a later line than the statement's first: the dynamic
# build knows statements' first lines only, so that warning is refused at run time, never placed wrong.
import warnings

x = len(str(
    warnings.warn("b")))
