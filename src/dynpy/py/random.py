# random for native builds (src/dynpy/mod.rs): CPython's Mersenne Twister (its core in rt_rand.rs), so a
# seeded program draws the same numbers as in CPython. Int seeds only (a str or float seed, which CPython hashes, is refused).
import math
import time
import _brevi_mt


class Random:
    def __init__(self, x=None):
        self._st = _brevi_mt.new()
        self.gauss_next = None
        self.seed(x)

    def seed(self, a=None, version=2):
        if a is None:
            a = int(time.time() * 1000000)
        if isinstance(a, bool) or not isinstance(a, int):
            raise TypeError("Brevi's own random takes an int seed (or none); add `# allow native` to seed with " + type(a).__name__)
        a = abs(a)
        key = []
        while a > 0:
            key.append(a & 0xFFFFFFFF)
            a = a >> 32
        _brevi_mt.seed(self._st, key)
        self.gauss_next = None

    def random(self):
        return _brevi_mt.random(self._st)

    def getrandbits(self, k):
        if k < 0:
            raise ValueError("number of bits must be non-negative")
        if k == 0:
            return 0
        if k <= 63:
            return _brevi_mt.bits(self._st, k)
        r = 0
        shift = 0
        while k > 0:
            w = _brevi_mt.next(self._st)
            if k < 32:
                w = w >> (32 - k)
            r = r | (w << shift)
            shift += 32
            k -= 32
        return r

    def _randbelow(self, n):
        if n < 9223372036854775807:
            return _brevi_mt.below(self._st, n)
        k = n.bit_length()
        r = self.getrandbits(k)
        while r >= n:
            r = self.getrandbits(k)
        return r

    def randrange(self, start, stop=None, step=1):
        if stop is None:
            if start > 0:
                return self._randbelow(start)
            raise ValueError("empty range for randrange()")
        width = stop - start
        if step == 1:
            if width > 0:
                return start + self._randbelow(width)
            raise ValueError("empty range in randrange(" + str(start) + ", " + str(stop) + ")")
        if step > 0:
            n = (width + step - 1) // step
        elif step < 0:
            n = (width + step + 1) // step
        else:
            raise ValueError("zero step for randrange()")
        if n <= 0:
            raise ValueError("empty range in randrange(" + str(start) + ", " + str(stop) + ", " + str(step) + ")")
        return start + step * self._randbelow(n)

    def randint(self, a, b):
        return self.randrange(a, b + 1)

    def choice(self, seq):
        if len(seq) == 0:
            raise IndexError("Cannot choose from an empty sequence")
        return seq[self._randbelow(len(seq))]

    def shuffle(self, x):
        if type(x) is list:
            _brevi_mt.shuffle(self._st, x)
            return
        i = len(x) - 1
        while i > 0:
            j = self._randbelow(i + 1)
            t = x[i]
            x[i] = x[j]
            x[j] = t
            i -= 1

    def sample(self, population, k):
        pool = list(population)
        n = len(pool)
        if k < 0 or k > n:
            raise ValueError("Sample larger than population or is negative")
        result = [None] * k
        setsize = 21
        if k > 5:
            setsize += 4 ** math.ceil(math.log(k * 3, 4))
        if n <= setsize:
            for i in range(k):
                j = self._randbelow(n - i)
                result[i] = pool[j]
                pool[j] = pool[n - i - 1]
        else:
            selected = set()
            for i in range(k):
                j = self._randbelow(n)
                while j in selected:
                    j = self._randbelow(n)
                selected.add(j)
                result[i] = pool[j]
        return result

    def choices(self, population, weights=None, cum_weights=None, k=1):
        n = len(population)
        if cum_weights is None:
            if weights is None:
                return [population[math.floor(self.random() * n)] for i in range(k)]
            cum_weights = []
            t = 0
            for w in weights:
                t = t + w
                cum_weights.append(t)
        elif weights is not None:
            raise TypeError("Cannot specify both weights and cumulative weights")
        if len(cum_weights) != n:
            raise ValueError("The number of weights does not match the population")
        total = cum_weights[-1] + 0.0
        if total <= 0.0:
            raise ValueError("Total of weights must be greater than zero")
        hi = n - 1
        out = []
        for i in range(k):
            x = self.random() * total
            lo = 0
            h = hi
            while lo < h:
                mid = (lo + h) // 2
                if x < cum_weights[mid]:
                    h = mid
                else:
                    lo = mid + 1
            out.append(population[lo])
        return out

    def uniform(self, a, b):
        return a + (b - a) * self.random()

    def gauss(self, mu=0.0, sigma=1.0):
        z = self.gauss_next
        self.gauss_next = None
        if z is None:
            x2pi = self.random() * (2.0 * math.pi)
            g2rad = math.sqrt(-2.0 * math.log(1.0 - self.random()))
            z = math.cos(x2pi) * g2rad
            self.gauss_next = math.sin(x2pi) * g2rad
        return mu + z * sigma


_inst = Random()
seed = _inst.seed
random = _inst.random
getrandbits = _inst.getrandbits
randrange = _inst.randrange
randint = _inst.randint
choice = _inst.choice
choices = _inst.choices
shuffle = _inst.shuffle
sample = _inst.sample
uniform = _inst.uniform
gauss = _inst.gauss
