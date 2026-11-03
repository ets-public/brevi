import sys
if sys.version_info[0] < 3:
    text = unicode
else:
    text = str
print(text(5))
if False:
    print(undefined_name)
print("a" + 1 if False else "ok")
