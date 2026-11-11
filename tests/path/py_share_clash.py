# This file binds a name the module binds too (kept): not followed.
from _share_keep import keep

kept = [0]
alias = kept
keep(kept)
alias.append(1)
print(kept)
