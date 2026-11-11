import math
import colorsys


def hue(r):
    return colorsys.rgb_to_hsv(r, 0.0, 0.0)[0]


print(hue(1.0), math.gcd(4, 6))
