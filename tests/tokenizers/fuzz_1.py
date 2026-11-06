# Random text through popular Hub tokenizers must give the ids, tokens, offsets and words CPython's tokenizers
# gives (seeded: the same text each run). Byte-level BPE (GPT-2, RoBERTa), a Llama-3-style split (Qwen2.5),
# SentencePiece-style BPE with byte fallback (TinyLlama) and WordPiece (BERT, MiniLM).
import random
from tokenizers import Tokenizer

ALPHA = list("abcXYZ019 _-.,!?'\"()[]{}<>/\\|@#$%^&*+=~`:;") + [
    "  ", "\n", "\t", "\r\n", " \n ", "'s", "'ll", "n't", "don't", "Hello", "world", "the", "ing", "##", "▁",
    "é", "ü", "ß", "À", "中文", "日本語", "한국어", "ñ", "ç", "́", "̈", "‍", "﻿", " ", "　",
    " ", "\x85", "\x00", "\x7f", "�", "İ", "K", "ſ", "ﬁ", "Ⅻ", "½", "٣", "𝐀", "😀", "👍🏽", "ǅ", "ʰ", "ः",
    "Ж", "א", "ا", "가", "가", "กิ", "க்", "", "１２３", "ｶ", "…", "—", "“", "”", "€",
    "[CLS]", "[SEP]", "[MASK]", "<s>", "</s>", "<unk>", "<|endoftext|>", "<|im_start|>",
]
rng = random.Random(1)
MODELS = [
    ("bert-base-uncased", Tokenizer.from_pretrained("bert-base-uncased")),
    ("gpt2", Tokenizer.from_pretrained("gpt2")),
    ("FacebookAI/roberta-base", Tokenizer.from_pretrained("FacebookAI/roberta-base")),
    ("Qwen/Qwen2.5-0.5B", Tokenizer.from_pretrained("Qwen/Qwen2.5-0.5B")),
    ("TinyLlama/TinyLlama-1.1B-Chat-v1.0", Tokenizer.from_pretrained("TinyLlama/TinyLlama-1.1B-Chat-v1.0")),
    ("sentence-transformers/all-MiniLM-L6-v2", Tokenizer.from_pretrained("sentence-transformers/all-MiniLM-L6-v2")),
]
for name, t in MODELS:
    h = 0
    for i in range(300):
        s = "".join(rng.choice(ALPHA) for _ in range(rng.randint(0, 30)))
        e = t.encode(s)
        words = [-1 if w is None else w for w in e.word_ids]
        for j, (x, (a, b), w) in enumerate(zip(e.ids, e.offsets, words)):
            h = (h * 1000003 + (j + 1) * (x * 7919 + a * 31 + b * 17 + w + 1)) % (1 << 61)
        d = t.decode(e.ids)
        h = (h * 1000003 + sum((j + 1) * ord(c) for j, c in enumerate(d))) % (1 << 61)
        if i % 60 == 0:
            print(name, repr(s), e.ids, e.offsets, repr(d))
    print(name, h)
