# Random text through each encoding must give the tokens CPython tiktoken gives (seeded: the same text each run).
import random
import tiktoken

ALPHA = list("abcXYZ019 '\n\r\t.,!?-_/\\\"()[]{}<>|") + [
    "'s", "'S", "'ll", "'LL", "'ve", "'re", "'d", "'m", "'t", "'ſ", "ſ", "K", "é", "É", "Ж", "ж", "中文", "ǅ", "ʰ",
    "́", "ः", "٣", "Ⅻ", "½", "😀", "👍🏽", " ", "　", "\x0b", "\x0c", "\x1c", "\x85", " ",
    "  ", "   ", "\n\n", " \n", "1234567", "ß", "İ", "ﬁ", "‍", "﻿", "\U0001d400", "א", "ا", "𝟘", "$", "€",
    "\x00", "\x7f", "ǈ", "<|endoftext|>", "<|", "|>",
]
rng = random.Random(1)
for name in ["r50k_base", "p50k_edit", "cl100k_base", "o200k_base", "o200k_harmony"]:
    enc = tiktoken.get_encoding(name)
    h = 0
    for i in range(400):
        s = "".join(rng.choice(ALPHA) for _ in range(rng.randint(0, 30)))
        toks = enc.encode(s, allowed_special="all")
        h = (h * 1000003 + sum((j + 1) * t for j, t in enumerate(toks))) % (1 << 61)
        if i % 50 == 0:
            print(name, repr(s), toks)
        assert enc.decode(toks) == s
    print(name, h)
