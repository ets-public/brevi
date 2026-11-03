import csv
import sys

def main():
    w = csv.writer(sys.stdout, lineterminator='\n')
    w.writerow(['id', 'name', 'note'])
    rows = [['1', 'a,b', 'say "hi"'], ['2', '', 'x\ny'], ['3', 'plain', ' sp ']]
    for r in rows:
        w.writerow(r)
    w.writerows([['4', 'four', 'z'], ['']])
    nums = [1, 2, 30]
    w.writerow(nums)
    print("(done)")
    v = csv.writer(sys.stdout, delimiter=';')
    v.writerow(['a;b', 'c'])

main()
def aliased(rows):
    out = sys.stdout
    w = csv.writer(out, lineterminator="\n")
    for r in rows:
        w.writerow(r)
aliased([["x", "y,z"], ["1", "2"]])
