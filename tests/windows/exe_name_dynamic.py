# Mixed-type values the typed build does not take: built on the dynamic path (exe_name.ps1 checks the path line).
xs = [1, "two", 3.0, None]
for x in xs:
    print(type(x).__name__, x)
