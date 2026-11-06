# tiktoken's OpenAI encodings: tokens, decoding and the encodings' facts, as CPython's tiktoken gives them.
import tiktoken

TEXTS = [
    "",
    "hello world",
    "Hello, world! It's 2026 and we're tokenizing.",
    "I'LL SEE YOU'RE DONE, he'd say. ſ'S K'D",
    "   leading and trailing spaces   ",
    "tabs\tand\nnewlines\r\n\r\nand\n\n\n   indented\n    code",
    "def f(x):\n    return x ** 2  # square\n",
    "numbers 1 12 123 1234 12345 3.14159 -42 1e10 ٣٤٥ Ⅻ ½",
    "naïve café résumé Zürich Ελληνικά русский 中文字符 日本語 한국어 עברית العربية",
    "emoji 😀👍🏽🇺🇸 and ZWJ 👩‍👩‍👧 sequences",
    "combining é ä and marks ःि",
    "ǅungla ǈ titlecase ʰmodifier",
    "path/to/file.py:12 https://example.com/a?b=c&d=e#frag",
    "<|endoftext|> is special but here it is plain text <|fim_prefix|>",
    " no-break　ideographic line para\x0bvt\x0cff\x1cfs\x85nel",
    "a" * 300,
    " " * 50 + "x",
    "!!!???...,,,;;;:::",
    "CamelCaseIdentifiers and snake_case_identifiers and SCREAMING_CASE",
    '{"json": [1, 2, 3], "key": "value", "nested": {"a": null}}',
]

for name in tiktoken.list_encoding_names():
    enc = tiktoken.get_encoding(name)
    print(enc, enc.name, enc.n_vocab, enc.max_token_value, enc.eot_token)
    print(" specials", len(enc.special_tokens_set), sorted(enc.special_tokens_set)[:5])
    total = 0
    for t in TEXTS:
        toks = enc.encode(t, disallowed_special=())
        total += len(toks)
        assert enc.decode(toks) == t, (name, t)
        assert enc.encode_ordinary(t) == toks
        if len(t) < 60:
            print(" ", len(toks), toks)
    print(" total", total)
    print(" bytes", enc.decode_tokens_bytes(enc.encode("Hello wörld")))
    print(" offsets", enc.decode_with_offsets(enc.encode("héllo wörld 中文")))
    print(" single", enc.encode_single_token("hello"), enc.encode_single_token(b" the"), enc.encode_single_token("<|endoftext|>"))
    print(" token", enc.decode_single_token_bytes(1000), enc.decode_bytes([1000, 1001, 1002]))
    print(" special", enc.encode("a<|endoftext|>b", allowed_special="all"), enc.encode("a<|endoftext|>b", allowed_special={"<|endoftext|>"}))
    print(" piece", enc._encode_single_piece("helloqqqq"), enc._encode_single_piece(b"zzzzzz"))
    print(" is_special", enc.is_special_token(enc.eot_token), enc.is_special_token(0))
    print(" batch", enc.encode_batch(["one", "two three"]), enc.decode_batch([[1, 2], [3]]), enc.encode_ordinary_batch(["x y"]))
    print(" bytes batch", enc.decode_bytes_batch([[1, 2], [3]]))
    vals = enc.token_byte_values()
    print(" values", len(vals), vals[:3], vals[-2:])

for model in ["gpt-4o", "gpt-4o-mini", "gpt-4", "gpt-4-0613", "gpt-3.5-turbo", "gpt-5", "gpt-5-mini", "o1", "o3-mini",
              "o4-mini-2025", "text-embedding-3-small", "text-davinci-003", "davinci", "gpt2", "gpt-oss-120b", "ft:gpt-4o:org"]:
    print(model, tiktoken.encoding_name_for_model(model), tiktoken.encoding_for_model(model).name)
print(tiktoken.__version__)
