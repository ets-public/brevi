# Brevi's tokenizers: the Python API of Hugging Face tokenizers 0.23.3 for a tokenizer loaded from its
# tokenizer.json (Tokenizer.from_file, from_pretrained, from_str), over a native core (the module _brevi_hf:
# src/dynpy/rt_hf.rs and rt_hfcore.rs, a port of tokenizers' Rust). A file or Hub tokenizer the program names
# by a literal is built into the program by the compiler, so nothing here reads a file or the network. The
# batch functions encode one input after another (tokenizers' thread pool returns the same lists). What this
# copy lacks (building a Tokenizer from models.*, training, the components Brevi has no port of) is refused
# when the program is built (src/caps.rs, src/dynpy/hf.rs), and CPython's tokenizers runs such a program.

import _brevi_hf

__version__ = "0.23.3"


class AddedToken:
    def __init__(self, content=None, **kwargs):
        self.content = "" if content is None else content
        self.special = False
        self._single_word = None
        self._lstrip = None
        self._rstrip = None
        self._normalized = None
        for key, value in kwargs.items():
            if key == "single_word":
                self._single_word = bool(value)
            elif key == "lstrip":
                self._lstrip = bool(value)
            elif key == "rstrip":
                self._rstrip = bool(value)
            elif key == "normalized":
                self._normalized = bool(value)
            elif key == "special":
                self.special = bool(value)
            else:
                print(f"Ignored unknown kwarg option {key}")

    @property
    def single_word(self):
        return bool(self._single_word)

    @property
    def lstrip(self):
        return bool(self._lstrip)

    @property
    def rstrip(self):
        return bool(self._rstrip)

    @property
    def normalized(self):
        return (not self.special) if self._normalized is None else self._normalized

    def _fields(self):
        return (self.content, self.single_word, self.lstrip, self.rstrip, self.normalized, self.special)

    def __str__(self):
        return self.content

    def __repr__(self):
        return (
            f'AddedToken("{self.content}", rstrip={self.rstrip}, lstrip={self.lstrip}, '
            f"single_word={self.single_word}, normalized={self.normalized}, special={self.special})"
        )

    def __eq__(self, other):
        if not isinstance(other, AddedToken):
            return NotImplemented
        return self._fields() == other._fields()

    def __hash__(self):
        return hash(self.content)


def _added(t):
    tok = AddedToken(t[1], single_word=t[2], lstrip=t[3], rstrip=t[4], normalized=t[5], special=t[6])
    return tok


def _token_list(tokens, special):
    if not isinstance(tokens, list):
        raise TypeError(f"'{type(tokens).__name__}' object cannot be converted to 'PyList'")
    out = []
    for t in tokens:
        if isinstance(t, str):
            out.append((t, False, False, False, not special, special))
        elif isinstance(t, AddedToken):
            if special:
                t.special = True
            out.append(t._fields())
        else:
            raise TypeError("Input must be a List[Union[str, AddedToken]]")
    return out


class _Core:
    def __init__(self, core):
        self.core = core


class Tokenizer:
    def __init__(self, model):
        # Tokenizer(models.X(...)) is refused when the program is built; this copy makes one from a core only
        if not isinstance(model, _Core):
            raise TypeError("Tokenizer() needs a tokenizers model")
        self._t = model.core

    @staticmethod
    def _wrap(core):
        return Tokenizer(_Core(core))

    @staticmethod
    def from_str(json):
        return Tokenizer._wrap(_brevi_hf.load(json))

    @staticmethod
    def from_file(path):
        return Tokenizer._wrap(_brevi_hf.load_built("file:" + path))

    @staticmethod
    def from_pretrained(identifier, revision="main", token=None):
        return Tokenizer._wrap(_brevi_hf.load_built("hub:" + identifier + "@" + revision))

    def encode(self, sequence, pair=None, is_pretokenized=False, add_special_tokens=True):
        return _brevi_hf.encode(self._t, sequence, pair, is_pretokenized, add_special_tokens, False)

    def encode_batch(self, input, is_pretokenized=False, add_special_tokens=True):
        return _brevi_hf.encode_batch(self._t, input, is_pretokenized, add_special_tokens, False)

    def encode_batch_fast(self, input, is_pretokenized=False, add_special_tokens=True):
        return _brevi_hf.encode_batch(self._t, input, is_pretokenized, add_special_tokens, True)

    def decode(self, ids, skip_special_tokens=True):
        return _brevi_hf.decode(self._t, ids, skip_special_tokens)

    def decode_batch(self, sequences, skip_special_tokens=True):
        return _brevi_hf.decode_batch(self._t, sequences, skip_special_tokens)

    def token_to_id(self, token):
        return _brevi_hf.token_to_id(self._t, token)

    def id_to_token(self, id):
        return _brevi_hf.id_to_token(self._t, id)

    def get_vocab(self, with_added_tokens=True):
        return _brevi_hf.vocab(self._t, with_added_tokens)

    def get_vocab_size(self, with_added_tokens=True):
        return _brevi_hf.vocab_size(self._t, with_added_tokens)

    def get_added_tokens_decoder(self):
        return {t[0]: _added(t) for t in _brevi_hf.added_tokens(self._t)}

    def num_special_tokens_to_add(self, is_pair):
        return _brevi_hf.n_added(self._t, is_pair)

    def add_tokens(self, tokens):
        return _brevi_hf.add_tokens(self._t, _token_list(tokens, False))

    def add_special_tokens(self, tokens):
        return _brevi_hf.add_tokens(self._t, _token_list(tokens, True))

    def enable_truncation(self, max_length, **kwargs):
        stride = 0
        strategy = "longest_first"
        direction = "right"
        for key, value in kwargs.items():
            if key == "stride":
                stride = value
            elif key == "strategy":
                strategy = value
            elif key == "direction":
                direction = value
            else:
                print(f"Ignored unknown kwarg option {key}")
        _brevi_hf.set_truncation(self._t, max_length, stride, strategy, direction)

    def no_truncation(self):
        _brevi_hf.set_truncation(self._t, None)

    @property
    def truncation(self):
        return _brevi_hf.truncation(self._t)

    def enable_padding(self, **kwargs):
        left = False
        pad_id = 0
        pad_type_id = 0
        pad_token = "[PAD]"
        length = None
        multiple = None
        for key, value in kwargs.items():
            if key == "direction":
                if not isinstance(value, str):
                    raise TypeError(f"'{type(value).__name__}' object is not an instance of 'str'")
                if value == "left":
                    left = True
                elif value == "right":
                    left = False
                else:
                    raise ValueError(f"Unknown `direction`: `{value}`. Use one of `left` or `right`")
            elif key == "pad_to_multiple_of":
                if value is not None:
                    multiple = value
            elif key == "pad_id":
                pad_id = value
            elif key == "pad_type_id":
                pad_type_id = value
            elif key == "pad_token":
                pad_token = value
            elif key == "max_length":
                print("enable_padding(max_length=X) is deprecated, use enable_padding(length=X) instead")
                length = value
            elif key == "length":
                length = value
            else:
                print(f"Ignored unknown kwarg option {key}")
        _brevi_hf.set_padding(self._t, left, pad_id, pad_type_id, pad_token, length, multiple)

    def no_padding(self):
        _brevi_hf.set_padding(self._t, None)

    @property
    def padding(self):
        return _brevi_hf.padding(self._t)

    @property
    def model(self):
        return _brevi_hf.lacks("Tokenizer.model")

    @property
    def encode_special_tokens(self):
        return _brevi_hf.encode_special(self._t)

    @encode_special_tokens.setter
    def encode_special_tokens(self, value):
        _brevi_hf.set_encode_special(self._t, value)
