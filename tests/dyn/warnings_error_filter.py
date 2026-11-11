# An "error" filter raises a warning wherever it is issued, so a warning from a call that opens on a later line
# of its statement (whose line the dynamic build does not know) is raised as in CPython; other actions refuse.
import re, warnings
warnings.simplefilter("error")
try:
    x = [1,
         warnings.warn("late", UserWarning)]
except UserWarning as e:
    print("raised", type(e).__name__, e)
try:
    y = (0,
         re.sub("a", "b", "aaa", 1))
except DeprecationWarning as e:
    print("raised", type(e).__name__, e)
warnings.filterwarnings("ignore", "late")
warnings.filterwarnings("error", "late", UserWarning, "__main__")
try:
    z = [2,
         warnings.warn("late", UserWarning)]
except UserWarning as e:
    print("raised again", e)
