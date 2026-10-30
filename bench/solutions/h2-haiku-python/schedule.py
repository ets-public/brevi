import sys

jobs = {}  # name -> (duration, deps)
order_names = []
for line in sys.stdin:
    parts = line.split()
    if not parts:
        continue
    name, dur = parts[0], int(parts[1])
    deps = []
    if len(parts) > 2:
        if parts[2] != 'after':
            print('invalid')
            sys.exit(0)
        deps = parts[3:]
    jobs[name] = (dur, deps)
    order_names.append(name)

for name, (dur, deps) in jobs.items():
    for d in deps:
        if d not in jobs:
            print('invalid')
            sys.exit(0)

start = {}
end = {}
visiting = set()

def compute(name):
    if name in end:
        return end[name]
    if name in visiting:
        raise ValueError('cycle')
    visiting.add(name)
    dur, deps = jobs[name]
    s = 0
    for d in deps:
        s = max(s, compute(d))
    visiting.discard(name)
    start[name] = s
    end[name] = s + dur
    return end[name]

try:
    for name in jobs:
        compute(name)
except ValueError:
    print('invalid')
    sys.exit(0)

for name in sorted(jobs, key=lambda n: (start[n], n)):
    print(f"{name} {start[name]} {end[name]}")
total = max(end.values()) if end else 0
print(f"total {total}")
