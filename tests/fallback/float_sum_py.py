# sum() of floats held as Python library values is compensated too (tests/typed/float_sum.py has the rest).
import colorsys

xs = [colorsys.rgb_to_hsv(0.1, 0.1, 0.1)[2] for _ in range(10)]
print(sum(xs))
print(sum(x for x in xs))
