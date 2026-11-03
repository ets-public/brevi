from enum import Enum
import json
class Suit(Enum):
    HEARTS = "H"
    SPADES = "S"
def score(cards):
    return sum(1 for c in cards if c == Suit.HEARTS)
cards = [Suit.HEARTS, Suit.SPADES, Suit("H")]
print(score(cards), cards[0].value, [c.name for c in Suit])
class Ordered(Enum):
    LOW = 1
    HIGH = 2
    def __lt__(self, other):
        return self.value < other.value
print(sorted([Ordered.HIGH, Ordered.LOW]), max(Ordered), vars(Ordered.LOW) if False else 0)
class K:
    b = 1
    a = 2
    def m(self): pass
print([k for k in vars(K) if not k.startswith('__')])
class Color(str, Enum):
    RED = "red"
    BLUE = "blue"
class Lvl(int, Enum):
    LO = 1
    HI = 5
print(Color.RED == "red", Color.RED.upper(), Color("blue"), f"{Color.RED}", str(Color.RED), repr(Color.RED))
print(Lvl.HI > Lvl.LO, Lvl.HI + 1, Lvl(5), sorted([Lvl.HI, Lvl.LO]), Lvl.LO == 1, {"red": 1}[Color.RED])
print(json.dumps({"c": Color.RED}))
