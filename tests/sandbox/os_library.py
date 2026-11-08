# os.listdir is Brevi's own and reads files: under --sandbox it needs allow read (before, CPython ran it and it needed allow native).
import os
print(os.listdir("."))
