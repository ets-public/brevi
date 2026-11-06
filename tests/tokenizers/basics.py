# Hub tokenizers (BERT's WordPiece, GPT-2's byte-level BPE) through the Tokenizer API AI code uses: encode and
# decode, pairs, batches, truncation and padding, added tokens, the Encoding's fields and position lookups.
from tokenizers import AddedToken, Tokenizer

bert = Tokenizer.from_pretrained("bert-base-uncased")
gpt2 = Tokenizer.from_pretrained("gpt2")
TEXTS = ["Hello, world!", "The quick brown fox jumps over the lazy dog.", "naïve café — résumé 中文 😀", "", "   ",
         "don't stop believin'", "def f(x):\n    return x ** 2  # square", "[CLS] tokens [SEP] in [MASK] text"]

for name, t in [("bert", bert), ("gpt2", gpt2)]:
    print("==", name, t.get_vocab_size(), t.get_vocab_size(with_added_tokens=False), t.num_special_tokens_to_add(False), t.num_special_tokens_to_add(True))
    for s in TEXTS:
        e = t.encode(s)
        print(repr(e), len(e), e.ids, e.tokens, e.offsets)
        print(" ", e.type_ids, e.attention_mask, e.special_tokens_mask, e.word_ids, e.sequence_ids, e.n_sequences, e.overflowing)
        print(" ", repr(t.decode(e.ids)), repr(t.decode(e.ids, skip_special_tokens=False)))
        if len(e):
            k = len(e) // 2
            print(" ", e.token_to_chars(k), e.token_to_word(k), e.token_to_sequence(k), e.char_to_token(1), e.char_to_word(1), e.word_to_tokens(0), e.word_to_chars(0))
    e = t.encode("first sentence here", "and a second one")
    print(e.ids, e.type_ids, e.sequence_ids, e.n_sequences, e.word_to_tokens(0, 1), e.char_to_token(0, 1), e.token_to_sequence(len(e) - 2))
    e = t.encode(["pre", "tokenized", "words!"], is_pretokenized=True)
    print(e.ids, e.tokens, e.offsets, e.word_ids)
    e = t.encode("no specials here", add_special_tokens=False)
    print(e.ids, e.tokens)
    for x in t.encode_batch(["one", ("two", "three"), ["four", "five"]]):
        print(x.ids, x.type_ids)
    for x in t.encode_batch_fast(["fast one", "fast two words"]):
        print(x.ids, x.tokens, x.offsets, x.word_ids)
    print(t.decode_batch([[t.token_to_id("the"), t.token_to_id("dog")], []]))
    print(t.token_to_id("hello"), t.token_to_id("not-a-token-xyz"), t.id_to_token(100), t.id_to_token(10**8))
    print(t.truncation, t.padding)
    t.enable_truncation(6)
    print(t.truncation, t.encode(TEXTS[1]).ids, [o.ids for o in t.encode(TEXTS[1]).overflowing])
    t.enable_truncation(13, stride=2, strategy="longest_first", direction="left")
    e = t.encode(TEXTS[1], TEXTS[0])
    print(t.truncation, e.ids, [o.ids for o in e.overflowing])
    t.enable_truncation(9, strategy="only_second")
    print(t.encode(TEXTS[0], TEXTS[1]).ids)
    t.enable_truncation(9, strategy="only_first")
    print(t.encode(TEXTS[1], TEXTS[0]).ids)
    t.no_truncation()
    t.enable_padding()
    print(t.padding, [x.ids for x in t.encode_batch(TEXTS[:3])], [x.attention_mask for x in t.encode_batch(TEXTS[:3])])
    t.enable_padding(direction="left", pad_id=7, pad_type_id=1, pad_token="<p>", length=12)
    e = t.encode(TEXTS[0], "pair")
    print(t.padding, e.ids, e.tokens, e.type_ids, e.special_tokens_mask, e.sequence_ids, e.word_to_tokens(0, 1))
    t.enable_padding(pad_to_multiple_of=8)
    print([len(x) for x in t.encode_batch(TEXTS)])
    t.enable_truncation(5)
    print([x.ids for x in t.encode_batch(TEXTS[:3])])
    t.no_padding()
    t.no_truncation()
    print(t.truncation, t.padding)
    print(t.add_tokens(["brevi", AddedToken("rustacean", single_word=True), "brevi"]), t.add_special_tokens(["<|tool|>", AddedToken("[ACT]", lstrip=True)]))
    for s in ["brevi is a rustacean", "brevirustacean <|tool|>call  [ACT] go", "a<|tool|>b"]:
        e = t.encode(s)
        print(e.ids, e.tokens, e.offsets, repr(t.decode(e.ids)), repr(t.decode(e.ids, skip_special_tokens=False)))
    print(t.get_vocab_size(), t.token_to_id("brevi"), t.id_to_token(t.token_to_id("<|tool|>")))
    dec = t.get_added_tokens_decoder()
    print(sorted(dec)[-4:], [repr(dec[k]) for k in sorted(dec)[-4:]])
    t.encode_special_tokens = True
    print(t.encode_special_tokens, t.encode("a<|tool|>b").tokens)
    t.encode_special_tokens = False
    vocab = t.get_vocab()
    print(len(vocab), vocab["brevi"], len(t.get_vocab(with_added_tokens=False)))
print(AddedToken("x"), repr(AddedToken("x", special=True)), repr(AddedToken("y", normalized=False, rstrip=True)), AddedToken("a") == AddedToken("a"))
