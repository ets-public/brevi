import sys

print(sys.get_int_max_str_digits())
for bad in (5, 639):
    try:
        sys.set_int_max_str_digits(bad)
    except ValueError as e:
        print("ValueError", e)
sys.set_int_max_str_digits(maxdigits=640)
print(sys.get_int_max_str_digits())
for s in ("1" * 640, "-" + "1" * 640, " +" + "1" * 640 + " ", "1_1" * 213):
    print(len(str(int(s))))
for s in ("1" * 641, "-" + "1" * 641):
    try:
        int(s)
    except ValueError as e:
        print("ValueError", e)
print(int("1" * 700, 16) % 1000, int("1" * 700, 2) % 1000)
try:
    int("2" * 641, 3)
except ValueError as e:
    print("ValueError", e)
try:
    str(10 ** 640)
except ValueError as e:
    print("ValueError", e)
print(len(str(10 ** 639)))
sys.set_int_max_str_digits(0)
print(len(str(10 ** 5000)), len(str(int("7" * 5000))))
