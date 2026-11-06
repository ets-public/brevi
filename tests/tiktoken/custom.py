# Custom tiktoken encodings: extending cl100k_base with chat tokens (tiktoken's README), and small vocabularies.
import tiktoken

cl100k_base = tiktoken.get_encoding("cl100k_base")
enc = tiktoken.Encoding(
    name="cl100k_im",
    pat_str=cl100k_base._pat_str,
    mergeable_ranks=cl100k_base._mergeable_ranks,
    special_tokens={
        **cl100k_base._special_tokens,
        "<|im_start|>": 100264,
        "<|im_end|>": 100265,
    },
)
msg = "<|im_start|>user\nWhat is 2+2?<|im_end|>\n<|im_start|>assistant\n"
toks = enc.encode(msg, allowed_special="all")
print(enc, enc.n_vocab, toks)
print(enc.decode(toks) == msg, enc.decode_tokens_bytes(toks[:4]))
print(enc.encode("hello world") == cl100k_base.encode("hello world"))

# a tiny byte-level BPE with the GPT-2 paper's pattern
ranks = {bytes([i]): i for i in range(256)}
for i, m in enumerate([b"th", b"the", b" t", b" the", b"in", b"ing", b"er", b"re"]):
    ranks[m] = 256 + i
small = tiktoken.Encoding(
    name="small",
    pat_str=r"""'s|'t|'re|'ve|'m|'ll|'d| ?\p{L}+| ?\p{N}+| ?[^\s\p{L}\p{N}]+|\s+(?!\S)|\s+""",
    mergeable_ranks=ranks,
    special_tokens={"<|end|>": 1000},
)
for s in ["the thing there", "ringing in the rear", "We're here, they'll see", "<|end|>", "1234 + 56"]:
    t = small.encode(s, allowed_special="all")
    print(repr(s), t, small.decode(t))
print(small.n_vocab, small.max_token_value, small.token_byte_values()[-3:])

# explicit_n_vocab is checked
try:
    tiktoken.Encoding("bad", pat_str=r"\S+|\s+", mergeable_ranks=ranks, special_tokens={}, explicit_n_vocab=5)
except AssertionError:
    print("AssertionError")
words = tiktoken.Encoding("words", pat_str=r"\S+|\s+", mergeable_ranks=ranks, special_tokens={}, explicit_n_vocab=264)
print(words.encode("the  ring   rereer"))
