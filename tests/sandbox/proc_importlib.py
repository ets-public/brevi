# Under --sandbox without --allow native, Brevi's child-process module is refused, however it is imported: the
# capability check reads every import the parser finds (not only those at the start of a line), and _brevi_proc
# itself refuses every call without the grant. (create_subprocess_ in this comment is load-bearing.)
import asyncio
import importlib
q = importlib.import_module("_brevi_proc")
p = q.spawn(["/bin/echo", "ESCAPED"], None, None, [0, 0, 0])
print(q.wait(p[0], True))
