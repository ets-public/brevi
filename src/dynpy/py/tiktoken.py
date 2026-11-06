# Brevi's tiktoken: the Python API of tiktoken 0.14 (core.py, registry.py, model.py and the OpenAI encodings of
# tiktoken_ext/openai_public.py), over a native core (the module _brevi_tok, src/dynpy/rt_tok.rs) in place of
# tiktoken's Rust extension. The OpenAI vocabularies are built into the program by the compiler, so nothing here
# downloads, reads a file or starts a thread. The batch functions encode one text after another (tiktoken's
# thread pool returns the same lists). What this copy lacks (encode_to_numpy, encode_with_unstable, tiktoken.load,
# plugins) is refused when the program is built (src/caps.rs), and CPython's tiktoken runs such a program.

import _brevi_tok

__version__ = "0.14.0"


class _Ranks:
    # the mergeable ranks of a vocabulary built into the program, made into a dict only when asked for
    def __init__(self, file):
        self.file = file


class Encoding:
    def __init__(self, name, *, pat_str, mergeable_ranks, special_tokens, explicit_n_vocab=None):
        self.name = name

        self._pat_str = pat_str
        self._special_tokens = special_tokens
        if isinstance(mergeable_ranks, _Ranks):
            self._core_bpe = _brevi_tok.core(mergeable_ranks.file, special_tokens, pat_str)
            self._ranks = None
            self.max_token_value = max(_brevi_tok.max_rank(self._core_bpe), max(special_tokens.values(), default=0))
            n_ranks = _brevi_tok.n_ranks(self._core_bpe)
        else:
            self._ranks = mergeable_ranks
            self.max_token_value = max(max(mergeable_ranks.values()), max(special_tokens.values(), default=0))
            n_ranks = len(mergeable_ranks)
        if explicit_n_vocab:
            assert n_ranks + len(special_tokens) == explicit_n_vocab
            assert self.max_token_value == explicit_n_vocab - 1

        self._special_token_values = set(self._special_tokens.values())
        self._special_set = None

        if not isinstance(mergeable_ranks, _Ranks):
            self._core_bpe = _brevi_tok.core(mergeable_ranks, special_tokens, pat_str)

    @property
    def _mergeable_ranks(self):
        if self._ranks is None:
            self._ranks = _brevi_tok.ranks(self._core_bpe)
        return self._ranks

    def __repr__(self):
        return f"<Encoding {self.name!r}>"

    # ====================
    # Encoding
    # ====================

    def encode_ordinary(self, text):
        return self._core_bpe.encode_ordinary(text)

    def _check(self, text, allowed_special, disallowed_special):
        if allowed_special == "all":
            allowed_special = self.special_tokens_set
        if disallowed_special == "all":
            disallowed_special = self.special_tokens_set - allowed_special
        if disallowed_special:
            if not isinstance(disallowed_special, frozenset):
                disallowed_special = frozenset(disallowed_special)
            match = _brevi_tok.find_special(text, disallowed_special)
            if match is not None:
                raise_disallowed_special_token(match)
        return allowed_special

    def encode(self, text, *, allowed_special=set(), disallowed_special="all"):
        allowed_special = self._check(text, allowed_special, disallowed_special)
        return self._core_bpe.encode(text, allowed_special)

    def encode_ordinary_batch(self, text, *, num_threads=8):
        return [self.encode_ordinary(t) for t in text]

    def encode_batch(self, text, *, num_threads=8, allowed_special=set(), disallowed_special="all"):
        if allowed_special == "all":
            allowed_special = self.special_tokens_set
        if disallowed_special == "all":
            disallowed_special = self.special_tokens_set - allowed_special
        if not isinstance(disallowed_special, frozenset):
            disallowed_special = frozenset(disallowed_special)
        return [self.encode(t, allowed_special=allowed_special, disallowed_special=disallowed_special) for t in text]

    def encode_single_token(self, text_or_bytes):
        if isinstance(text_or_bytes, str):
            text_or_bytes = text_or_bytes.encode("utf-8")
        return self._core_bpe.encode_single_token(text_or_bytes)

    # ====================
    # Decoding
    # ====================

    def decode_bytes(self, tokens):
        return self._core_bpe.decode_bytes(tokens)

    def decode(self, tokens, errors="replace"):
        return self._core_bpe.decode_bytes(tokens).decode("utf-8", errors=errors)

    def decode_single_token_bytes(self, token):
        return self._core_bpe.decode_single_token_bytes(token)

    def decode_tokens_bytes(self, tokens):
        return [self.decode_single_token_bytes(token) for token in tokens]

    def decode_with_offsets(self, tokens):
        token_bytes = self.decode_tokens_bytes(tokens)

        text_len = 0
        offsets = []
        for token in token_bytes:
            offsets.append(max(0, text_len - (0x80 <= token[0] < 0xC0)))
            text_len += sum(1 for c in token if not 0x80 <= c < 0xC0)

        text = b"".join(token_bytes).decode("utf-8", errors="strict")
        return text, offsets

    def decode_batch(self, batch, *, errors="replace", num_threads=8):
        return [self.decode(t, errors=errors) for t in batch]

    def decode_bytes_batch(self, batch, *, num_threads=8):
        return [self.decode_bytes(t) for t in batch]

    # ====================
    # Miscellaneous
    # ====================

    def token_byte_values(self):
        return self._core_bpe.token_byte_values()

    @property
    def eot_token(self):
        return self._special_tokens["<|endoftext|>"]

    @property
    def special_tokens_set(self):
        # functools.cached_property in tiktoken
        if self._special_set is None:
            self._special_set = set(self._special_tokens.keys())
        return self._special_set

    def is_special_token(self, token):
        assert isinstance(token, int)
        return token in self._special_token_values

    @property
    def n_vocab(self):
        return self.max_token_value + 1

    # ====================
    # Private
    # ====================

    def _encode_single_piece(self, text_or_bytes):
        if isinstance(text_or_bytes, str):
            text_or_bytes = text_or_bytes.encode("utf-8")
        return self._core_bpe.encode_single_piece(text_or_bytes)


def raise_disallowed_special_token(token):
    raise ValueError(
        f"Encountered text corresponding to disallowed special token {token!r}.\n"
        "If you want this text to be encoded as a special token, "
        f"pass it to `allowed_special`, e.g. `allowed_special={{{token!r}, ...}}`.\n"
        f"If you want this text to be encoded as normal text, disable the check for this token "
        f"by passing `disallowed_special=(enc.special_tokens_set - {{{token!r}}})`.\n"
        "To disable this check for all special tokens, pass `disallowed_special=()`.\n"
    )


# ====================
# tiktoken_ext/openai_public.py
# ====================

ENDOFTEXT = "<|endoftext|>"
FIM_PREFIX = "<|fim_prefix|>"
FIM_MIDDLE = "<|fim_middle|>"
FIM_SUFFIX = "<|fim_suffix|>"
ENDOFPROMPT = "<|endofprompt|>"

r50k_pat_str = r"""'(?:[sdmt]|ll|ve|re)| ?\p{L}++| ?\p{N}++| ?[^\s\p{L}\p{N}]++|\s++$|\s+(?!\S)|\s"""


def _gpt2():
    # tiktoken reads GPT-2's encoder.json and vocab.bpe: the same ranks as r50k_base's file
    return {
        "name": "gpt2",
        "explicit_n_vocab": 50257,
        "pat_str": r50k_pat_str,
        "mergeable_ranks": _Ranks("r50k_base"),
        "special_tokens": {ENDOFTEXT: 50256},
    }


def _r50k_base():
    return {
        "name": "r50k_base",
        "explicit_n_vocab": 50257,
        "pat_str": r50k_pat_str,
        "mergeable_ranks": _Ranks("r50k_base"),
        "special_tokens": {ENDOFTEXT: 50256},
    }


def _p50k_base():
    return {
        "name": "p50k_base",
        "explicit_n_vocab": 50281,
        "pat_str": r50k_pat_str,
        "mergeable_ranks": _Ranks("p50k_base"),
        "special_tokens": {ENDOFTEXT: 50256},
    }


def _p50k_edit():
    special_tokens = {ENDOFTEXT: 50256, FIM_PREFIX: 50281, FIM_MIDDLE: 50282, FIM_SUFFIX: 50283}
    return {
        "name": "p50k_edit",
        "pat_str": r50k_pat_str,
        "mergeable_ranks": _Ranks("p50k_base"),
        "special_tokens": special_tokens,
    }


def _cl100k_base():
    special_tokens = {
        ENDOFTEXT: 100257,
        FIM_PREFIX: 100258,
        FIM_MIDDLE: 100259,
        FIM_SUFFIX: 100260,
        ENDOFPROMPT: 100276,
    }
    return {
        "name": "cl100k_base",
        "pat_str": r"""'(?i:[sdmt]|ll|ve|re)|[^\r\n\p{L}\p{N}]?+\p{L}++|\p{N}{1,3}+| ?[^\s\p{L}\p{N}]++[\r\n]*+|\s++$|\s*[\r\n]|\s+(?!\S)|\s""",
        "mergeable_ranks": _Ranks("cl100k_base"),
        "special_tokens": special_tokens,
    }


def _o200k_base():
    special_tokens = {ENDOFTEXT: 199999, ENDOFPROMPT: 200018}
    pat_str = "|".join(
        [
            r"""[^\r\n\p{L}\p{N}]?[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]*[\p{Ll}\p{Lm}\p{Lo}\p{M}]+(?i:'s|'t|'re|'ve|'m|'ll|'d)?""",
            r"""[^\r\n\p{L}\p{N}]?[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]+[\p{Ll}\p{Lm}\p{Lo}\p{M}]*(?i:'s|'t|'re|'ve|'m|'ll|'d)?""",
            r"""\p{N}{1,3}""",
            r""" ?[^\s\p{L}\p{N}]+[\r\n/]*""",
            r"""\s*[\r\n]+""",
            r"""\s+(?!\S)""",
            r"""\s+""",
        ]
    )
    return {
        "name": "o200k_base",
        "pat_str": pat_str,
        "mergeable_ranks": _Ranks("o200k_base"),
        "special_tokens": special_tokens,
    }


def _o200k_harmony():
    base_enc = _o200k_base()
    name = "o200k_harmony"
    pat_str = base_enc["pat_str"]
    mergeable_ranks = base_enc["mergeable_ranks"]
    special_tokens = {
        **base_enc["special_tokens"],
        "<|startoftext|>": 199998,
        "<|endoftext|>": 199999,
        "<|reserved_200000|>": 200000,
        "<|reserved_200001|>": 200001,
        "<|return|>": 200002,
        "<|constrain|>": 200003,
        "<|reserved_200004|>": 200004,
        "<|channel|>": 200005,
        "<|start|>": 200006,
        "<|end|>": 200007,
        "<|message|>": 200008,
        "<|reserved_200009|>": 200009,
        "<|reserved_200010|>": 200010,
        "<|reserved_200011|>": 200011,
        "<|call|>": 200012,
    } | {f"<|reserved_{i}|>": i for i in range(200013, 201088)}
    return {
        "name": name,
        "pat_str": pat_str,
        "mergeable_ranks": mergeable_ranks,
        "special_tokens": special_tokens,
    }


ENCODING_CONSTRUCTORS = {
    "gpt2": _gpt2,
    "r50k_base": _r50k_base,
    "p50k_base": _p50k_base,
    "p50k_edit": _p50k_edit,
    "cl100k_base": _cl100k_base,
    "o200k_base": _o200k_base,
    "o200k_harmony": _o200k_harmony,
}

# ====================
# registry.py
# ====================

ENCODINGS = {}


def get_encoding(encoding_name):
    if not isinstance(encoding_name, str):
        raise ValueError(f"Expected a string in get_encoding, got {type(encoding_name)}")

    if encoding_name in ENCODINGS:
        return ENCODINGS[encoding_name]

    if encoding_name not in ENCODING_CONSTRUCTORS:
        raise ValueError(
            f"Unknown encoding {encoding_name}.\n"
            f"Plugins found: ['tiktoken_ext.openai_public']\n"
            f"tiktoken version: {__version__} (are you on latest?)"
        )

    constructor = ENCODING_CONSTRUCTORS[encoding_name]
    enc = Encoding(**constructor())
    ENCODINGS[encoding_name] = enc
    return enc


def list_encoding_names():
    return list(ENCODING_CONSTRUCTORS)


# ====================
# model.py
# ====================

MODEL_PREFIX_TO_ENCODING = {
    "o1-": "o200k_base",
    "o3-": "o200k_base",
    "o4-mini-": "o200k_base",
    # chat
    "gpt-5": "o200k_base",
    "gpt-4.5-": "o200k_base",
    "gpt-4.1-": "o200k_base",
    "chatgpt-4o-": "o200k_base",
    "gpt-4o-": "o200k_base",
    "gpt-4-": "cl100k_base",
    "gpt-3.5-turbo-": "cl100k_base",
    "gpt-35-turbo-": "cl100k_base",
    "gpt-oss-": "o200k_harmony",
    # fine-tuned
    "ft:gpt-4o": "o200k_base",
    "ft:gpt-4": "cl100k_base",
    "ft:gpt-3.5-turbo": "cl100k_base",
    "ft:davinci-002": "cl100k_base",
    "ft:babbage-002": "cl100k_base",
}

MODEL_TO_ENCODING = {
    # reasoning
    "o1": "o200k_base",
    "o3": "o200k_base",
    "o4-mini": "o200k_base",
    # chat
    "gpt-5": "o200k_base",
    "gpt-4.1": "o200k_base",
    "gpt-4o": "o200k_base",
    "gpt-4": "cl100k_base",
    "gpt-3.5-turbo": "cl100k_base",
    "gpt-3.5": "cl100k_base",
    "gpt-35-turbo": "cl100k_base",
    # base
    "davinci-002": "cl100k_base",
    "babbage-002": "cl100k_base",
    # embeddings
    "text-embedding-ada-002": "cl100k_base",
    "text-embedding-3-small": "cl100k_base",
    "text-embedding-3-large": "cl100k_base",
    # DEPRECATED MODELS
    # text (DEPRECATED)
    "text-davinci-003": "p50k_base",
    "text-davinci-002": "p50k_base",
    "text-davinci-001": "r50k_base",
    "text-curie-001": "r50k_base",
    "text-babbage-001": "r50k_base",
    "text-ada-001": "r50k_base",
    "davinci": "r50k_base",
    "curie": "r50k_base",
    "babbage": "r50k_base",
    "ada": "r50k_base",
    # code (DEPRECATED)
    "code-davinci-002": "p50k_base",
    "code-davinci-001": "p50k_base",
    "code-cushman-002": "p50k_base",
    "code-cushman-001": "p50k_base",
    "davinci-codex": "p50k_base",
    "cushman-codex": "p50k_base",
    # edit (DEPRECATED)
    "text-davinci-edit-001": "p50k_edit",
    "code-davinci-edit-001": "p50k_edit",
    # old embeddings (DEPRECATED)
    "text-similarity-davinci-001": "r50k_base",
    "text-similarity-curie-001": "r50k_base",
    "text-similarity-babbage-001": "r50k_base",
    "text-similarity-ada-001": "r50k_base",
    "text-search-davinci-doc-001": "r50k_base",
    "text-search-curie-doc-001": "r50k_base",
    "text-search-babbage-doc-001": "r50k_base",
    "text-search-ada-doc-001": "r50k_base",
    "code-search-babbage-code-001": "r50k_base",
    "code-search-ada-code-001": "r50k_base",
    # open source
    "gpt2": "gpt2",
    "gpt-2": "gpt2",
}


def encoding_name_for_model(model_name):
    if model_name in MODEL_TO_ENCODING:
        return MODEL_TO_ENCODING[model_name]

    for model_prefix, encoding_name in MODEL_PREFIX_TO_ENCODING.items():
        if model_name.startswith(model_prefix):
            return encoding_name

    raise KeyError(
        f"Could not automatically map {model_name} to a tokeniser. "
        "Please use `tiktoken.get_encoding` to explicitly get the tokeniser you expect."
    ) from None


def encoding_for_model(model_name):
    return get_encoding(encoding_name_for_model(model_name))
