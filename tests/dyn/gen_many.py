# Regression test for rt_gen's live-coroutine list: every finished generator must release its slot for reuse,
# so starting 2,000,000 short generators one after another does not grow the list (and its memory) by one entry
# per generator for the life of the process. This test checks the run finishes and the sum is right; the slot
# reuse is verified separately with /proc/self/status VmHWM (peak RSS stays near the one-generator baseline
# instead of climbing by ~16 MB).
def gen():
    yield 1

n = 0
for _ in range(2000000):
    for x in gen():
        n += x
print(n)
