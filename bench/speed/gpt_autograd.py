# A tiny character-level GPT trained with a scalar autograd engine, in plain Python.
# Written for Brevi's speed work: it does what Karpathy's microgpt does (one Value object per
# number, a tuple of children and local gradients per node, a recursive topological sort for the
# backward pass, attention over a growing key/value cache, Adam), so its time profile is the same.
# Usage: python3 gpt_autograd.py [steps]   (the words are made here, so no data file is needed)
import math
import random
import sys

random.seed(7)
STEPS = int(sys.argv[1]) if len(sys.argv) > 1 else 300

# words built from syllables, so the model has something to learn
syl = ["ka", "ri", "mo", "an", "el", "ta", "jo", "su", "ne", "li", "ba", "or", "vi", "de"]
words = []
for _ in range(2000):
    words.append("".join(random.choice(syl) for _ in range(random.randint(1, 4))))
random.shuffle(words)
chars = sorted(set("".join(words)))
EOS = len(chars)
nvocab = len(chars) + 1
print("words:", len(words), "vocab:", nvocab)


class Node:
    __slots__ = ("val", "grad", "kids", "dkids")

    def __init__(self, val, kids=(), dkids=()):
        self.val = val
        self.grad = 0
        self.kids = kids
        self.dkids = dkids

    def __add__(self, o):
        o = o if isinstance(o, Node) else Node(o)
        return Node(self.val + o.val, (self, o), (1, 1))

    def __mul__(self, o):
        o = o if isinstance(o, Node) else Node(o)
        return Node(self.val * o.val, (self, o), (o.val, self.val))

    def __pow__(self, k):
        return Node(self.val**k, (self,), (k * self.val ** (k - 1),))

    def log(self):
        return Node(math.log(self.val), (self,), (1 / self.val,))

    def exp(self):
        e = math.exp(self.val)
        return Node(e, (self,), (e,))

    def relu(self):
        return Node(max(0, self.val), (self,), (float(self.val > 0),))

    def __neg__(self):
        return self * -1

    def __radd__(self, o):
        return self + o

    def __sub__(self, o):
        return self + (-o)

    def __rsub__(self, o):
        return o + (-self)

    def __rmul__(self, o):
        return self * o

    def __truediv__(self, o):
        return self * o**-1

    def __rtruediv__(self, o):
        return o * self**-1

    def backward(self):
        order = []
        seen = set()

        def visit(n):
            if n not in seen:
                seen.add(n)
                for k in n.kids:
                    visit(k)
                order.append(n)

        visit(self)
        self.grad = 1
        for n in reversed(order):
            for k, d in zip(n.kids, n.dkids):
                k.grad += d * n.grad


D = 16
CTX = 16
HEADS = 4
HD = D // HEADS


def mat(rows, cols, std=0.08):
    return [[Node(random.gauss(0, std)) for _ in range(cols)] for _ in range(rows)]


W = {
    "tok": mat(nvocab, D),
    "pos": mat(CTX, D),
    "out": mat(nvocab, D),
    "q": mat(D, D),
    "k": mat(D, D),
    "v": mat(D, D),
    "o": mat(D, D),
    "up": mat(4 * D, D),
    "down": mat(D, 4 * D),
}
params = [p for m in W.values() for row in m for p in row]
print("params:", len(params))


def matvec(x, w):
    return [sum(a * b for a, b in zip(row, x)) for row in w]


def softmax(xs):
    top = max(x.val for x in xs)
    ex = [(x - top).exp() for x in xs]
    s = sum(ex)
    return [e / s for e in ex]


def norm(x):
    ms = sum(a * a for a in x) / len(x)
    r = (ms + 1e-5) ** -0.5
    return [a * r for a in x]


def model(tok, pos, ks, vs):
    x = [a + b for a, b in zip(W["tok"][tok], W["pos"][pos])]
    x = norm(x)
    res = x
    x = norm(x)
    q = matvec(x, W["q"])
    ks.append(matvec(x, W["k"]))
    vs.append(matvec(x, W["v"]))
    att = []
    for h in range(HEADS):
        s = h * HD
        qh = q[s : s + HD]
        kh = [k[s : s + HD] for k in ks]
        vh = [v[s : s + HD] for v in vs]
        logits = [sum(qh[j] * kh[t][j] for j in range(HD)) / HD**0.5 for t in range(len(kh))]
        w = softmax(logits)
        att.extend([sum(w[t] * vh[t][j] for t in range(len(vh))) for j in range(HD)])
    x = matvec(att, W["o"])
    x = [a + b for a, b in zip(x, res)]
    res = x
    x = norm(x)
    x = [a.relu() for a in matvec(x, W["up"])]
    x = matvec(x, W["down"])
    x = [a + b for a, b in zip(x, res)]
    return matvec(x, W["out"])


lr, b1, b2, eps = 0.01, 0.85, 0.99, 1e-8
m1 = [0.0] * len(params)
m2 = [0.0] * len(params)
for step in range(STEPS):
    word = words[step % len(words)]
    toks = [EOS] + [chars.index(c) for c in word] + [EOS]
    n = min(CTX, len(toks) - 1)
    ks, vs = [], []
    losses = []
    for p in range(n):
        probs = softmax(model(toks[p], p, ks, vs))
        losses.append(-probs[toks[p + 1]].log())
    loss = (1 / n) * sum(losses)
    loss.backward()
    lrt = lr * (1 - step / STEPS)
    for i, p in enumerate(params):
        m1[i] = b1 * m1[i] + (1 - b1) * p.grad
        m2[i] = b2 * m2[i] + (1 - b2) * p.grad**2
        mh = m1[i] / (1 - b1 ** (step + 1))
        vh = m2[i] / (1 - b2 ** (step + 1))
        p.val -= lrt * mh / (vh**0.5 + eps)
        p.grad = 0
    if step % 50 == 0 or step == STEPS - 1:
        print(f"step {step + 1:4d} loss {loss.val:.4f}")

print("samples:")
for _ in range(8):
    ks, vs = [], []
    t = EOS
    out = []
    for p in range(CTX):
        probs = softmax([l / 0.5 for l in model(t, p, ks, vs)])
        t = random.choices(range(nvocab), weights=[q.val for q in probs])[0]
        if t == EOS:
            break
        out.append(chars[t])
    print("".join(out))
