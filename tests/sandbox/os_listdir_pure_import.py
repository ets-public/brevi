# Path functions run without allow native, but file and process calls in the same program are still refused.
import os
from os import system
print(os.path.join("a", "b"))
print(os.listdir("."))
system("echo escaped the sandbox")
