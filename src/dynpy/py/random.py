# random for sandboxed builds (src/dynpy/mod.rs): CPython's Mersenne Twister, so a seeded program draws the
# same numbers as in CPython. Int seeds only (a str or float seed, which CPython hashes, is refused).
import math
import time


class Random:
    def __init__(self, x=None):
        self._mt = [0] * 624
        self._i = 624
        self.gauss_next = None
        self.seed(x)

    def _init_genrand(self, s):
        mt = self._mt
        mt[0] = s & 0xFFFFFFFF
        for i in range(1, 624):
            p = mt[i - 1]
            mt[i] = (1812433253 * (p ^ (p >> 30)) + i) & 0xFFFFFFFF
        self._i = 624

    def seed(self, a=None, version=2):
        if a is None:
            a = int(time.time() * 1000000)
        if isinstance(a, bool) or not isinstance(a, int):
            raise TypeError("Brevi's sandboxed random takes an int seed (or none); add `# allow native` to seed with " + type(a).__name__)
        a = abs(a)
        key = []
        while a > 0:
            key.append(a & 0xFFFFFFFF)
            a = a >> 32
        if len(key) == 0:
            key = [0]
        self._init_genrand(19650218)
        mt = self._mt
        i = 1
        j = 0
        k = 624
        if len(key) > k:
            k = len(key)
        while k > 0:
            p = mt[i - 1]
            mt[i] = ((mt[i] ^ ((p ^ (p >> 30)) * 1664525)) + key[j] + j) & 0xFFFFFFFF
            i += 1
            j += 1
            if i >= 624:
                mt[0] = mt[623]
                i = 1
            if j >= len(key):
                j = 0
            k -= 1
        k = 623
        while k > 0:
            p = mt[i - 1]
            mt[i] = ((mt[i] ^ ((p ^ (p >> 30)) * 1566083941)) - i) & 0xFFFFFFFF
            i += 1
            if i >= 624:
                mt[0] = mt[623]
                i = 1
            k -= 1
        mt[0] = 0x80000000
        self._i = 624
        self.gauss_next = None

    def _next(self):
        mt = self._mt
        if self._i >= 624:
            for kk in range(624):
                y = (mt[kk] & 0x80000000) | (mt[(kk + 1) % 624] & 0x7FFFFFFF)
                v = mt[(kk + 397) % 624] ^ (y >> 1)
                if y & 1:
                    v = v ^ 0x9908B0DF
                mt[kk] = v
            self._i = 0
        y = mt[self._i]
        self._i += 1
        y = y ^ (y >> 11)
        y = y ^ ((y << 7) & 0x9D2C5680)
        y = y ^ ((y << 15) & 0xEFC60000)
        y = y ^ (y >> 18)
        return y

    def random(self):
        a = self._next() >> 5
        b = self._next() >> 6
        return (a * 67108864.0 + b) * (1.0 / 9007199254740992.0)

    def getrandbits(self, k):
        if k < 0:
            raise ValueError("number of bits must be non-negative")
        if k == 0:
            return 0
        if k <= 32:
            return self._next() >> (32 - k)
        r = 0
        shift = 0
        while k > 0:
            w = self._next()
            if k < 32:
                w = w >> (32 - k)
            r = r | (w << shift)
            shift += 32
            k -= 32
        return r

    def _randbelow(self, n):
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
