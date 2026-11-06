# Tokenizers built from tokenizer.json text (Tokenizer.from_str): each component Brevi's tokenizers has, on
# texts that reach its corners. Prints what CPython's tokenizers prints.
import json
from tokenizers import Tokenizer

VOCAB = ["<unk>", "<s>", "</s>", "[CLS]", "[SEP]", "[PAD]", "[MASK]"] + list("abcdefghijklmnopqrstuvwxyz0123456789.,!?'-_ ") + [
    "▁", "▁a", "▁the", "th", "he", "the", "in", "ing", "▁in", "an", "and", "er", "re", "▁re", "on", "es", "##s", "##ing",
    "hello", "world", "▁hello", "▁world", "<0x41>", "<0xC3>", "<0xA9>", "<0xE4>", "<0xB8>", "<0xAD>", "</w>", "a</w>", "##e",
    "Ġ", "Ġt", "Ġth", "Ġthe", "ĠĠ", "Ċ", "é", "x", "y", "z",
]
VOCAB = list(dict.fromkeys(VOCAB))
IDS = {t: i for i, t in enumerate(VOCAB)}


def merges(pairs):
    out = []
    for a, b in pairs:
        if a in IDS and b in IDS and a + b in IDS:
            out.append(a + " " + b)
    return out


BPE_MERGES = merges([("t", "h"), ("h", "e"), ("th", "e"), ("i", "n"), ("in", "g"), ("a", "n"), ("an", "d"), ("e", "r"), ("r", "e"),
                     ("o", "n"), ("e", "s"), ("▁", "a"), ("▁", "the"), ("▁", "in"), ("▁", "re"), ("Ġ", "t"), ("Ġt", "h"), ("Ġth", "e"), ("Ġ", "Ġ")])


def bpe(**kw):
    m = {"type": "BPE", "dropout": None, "unk_token": "<unk>", "continuing_subword_prefix": None, "end_of_word_suffix": None,
         "fuse_unk": False, "byte_fallback": False, "ignore_merges": False, "vocab": IDS, "merges": BPE_MERGES}
    m.update(kw)
    return m


def tok(model, normalizer=None, pre=None, post=None, decoder=None, added=()):
    added_tokens = []
    for i, (content, special, extra) in enumerate(added):
        t = {"id": IDS.get(content, len(VOCAB) + i), "content": content, "single_word": False, "lstrip": False, "rstrip": False,
             "normalized": not special, "special": special}
        t.update(extra)
        added_tokens.append(t)
    return Tokenizer.from_str(json.dumps({"version": "1.0", "truncation": None, "padding": None, "added_tokens": added_tokens,
                                          "normalizer": normalizer, "pre_tokenizer": pre, "post_processor": post, "decoder": decoder,
                                          "model": model}))


TEXTS = ["hello world", "  the thing  in the  ring ", "Héllo, wörld! The END.", "a1b22c333 4444", "re-entering the_ring?",
         "", " ", "中文 and 日本", "x\ty\nz\r\n  end", "ﬁ ½ Ⅻ ①", "don't  re'enter"]


def show(name, t, texts=TEXTS, pair=None):
    print("==", name)
    for s in texts:
        e = t.encode(s) if pair is None else t.encode(s, pair)
        print(repr(s), e.ids, e.tokens, e.offsets, e.word_ids, repr(t.decode(e.ids)), repr(t.decode(e.ids, skip_special_tokens=False)))


ws = {"type": "WhitespaceSplit"}
show("bpe whitespace", tok(bpe(), pre=ws))
show("bpe fuse_unk", tok(bpe(fuse_unk=True), pre={"type": "Whitespace"}))
show("bpe byte_fallback", tok(bpe(byte_fallback=True), pre=ws, decoder={"type": "Sequence", "decoders": [{"type": "ByteFallback"}, {"type": "Fuse"}]}))
show("bpe prefix suffix", tok(bpe(continuing_subword_prefix="##", end_of_word_suffix="</w>", merges=[]), pre=ws, decoder={"type": "BPEDecoder", "suffix": "</w>"}))
show("bpe ignore_merges", tok(bpe(ignore_merges=True), pre=ws))
show("bpe no unk", tok(bpe(unk_token=None), pre={"type": "BertPreTokenizer"}))
for scheme in ["always", "first", "never"]:
    for split in [True, False]:
        mt = {"type": "Metaspace", "replacement": "▁", "prepend_scheme": scheme, "split": split}
        show(f"metaspace {scheme} {split}", tok(bpe(), pre=mt, decoder={"type": "Metaspace", "replacement": "▁", "prepend_scheme": scheme, "split": split}))
show("llama-style", tok(bpe(byte_fallback=True, fuse_unk=True),
                        normalizer={"type": "Sequence", "normalizers": [{"type": "Prepend", "prepend": "▁"}, {"type": "Replace", "pattern": {"String": " "}, "content": "▁"}]},
                        decoder={"type": "Sequence", "decoders": [{"type": "Replace", "pattern": {"String": "▁"}, "content": " "}, {"type": "ByteFallback"}, {"type": "Fuse"}, {"type": "Strip", "content": " ", "start": 1, "stop": 0}]},
                        added=[("<s>", True, {}), ("</s>", True, {})]),
     texts=TEXTS + ["<s>hello</s> the <s> end"])
show("bytelevel", tok(bpe(), pre={"type": "ByteLevel", "add_prefix_space": True, "trim_offsets": True, "use_regex": True},
                      post={"type": "ByteLevel", "add_prefix_space": True, "trim_offsets": True, "use_regex": True}, decoder={"type": "ByteLevel", "add_prefix_space": True, "trim_offsets": True, "use_regex": True}))
show("bytelevel no prefix", tok(bpe(), pre={"type": "ByteLevel", "add_prefix_space": False, "trim_offsets": False, "use_regex": False}, decoder={"type": "ByteLevel", "add_prefix_space": True, "trim_offsets": True, "use_regex": True}))
for beh in ["Removed", "Isolated", "MergedWithPrevious", "MergedWithNext", "Contiguous"]:
    for inv in [False, True]:
        sp = {"type": "Split", "pattern": {"Regex": "[aeiou]+| +"}, "behavior": beh, "invert": inv}
        show(f"split {beh} {inv}", tok(bpe(), pre=sp), texts=TEXTS[:5])
show("split string", tok(bpe(), pre={"type": "Split", "pattern": {"String": "th"}, "behavior": "Isolated", "invert": False}), texts=TEXTS[:5])
show("digits", tok(bpe(), pre={"type": "Sequence", "pretokenizers": [ws, {"type": "Digits", "individual_digits": False}]}))
show("digits single", tok(bpe(), pre={"type": "Digits", "individual_digits": True}))
show("punctuation", tok(bpe(), pre={"type": "Sequence", "pretokenizers": [ws, {"type": "Punctuation", "behavior": "Isolated"}]}))
show("char delimiter", tok(bpe(), pre={"type": "CharDelimiterSplit", "delimiter": "e"}))
show("normalizers", tok(bpe(), normalizer={"type": "Sequence", "normalizers": [{"type": "NFKC"}, {"type": "Lowercase"}, {"type": "StripAccents"}, {"type": "Strip", "strip_left": True, "strip_right": True}]}, pre=ws))
show("nfd strip", tok(bpe(), normalizer={"type": "Sequence", "normalizers": [{"type": "NFD"}, {"type": "StripAccents"}, {"type": "Strip", "strip_left": False, "strip_right": True}]}, pre=ws))
show("nfkd lowercase", tok(bpe(), normalizer={"type": "Sequence", "normalizers": [{"type": "NFKD"}, {"type": "Lowercase"}]}, pre=ws))
show("bytelevel normalizer", tok(bpe(), normalizer={"type": "ByteLevel", "add_prefix_space": True, "trim_offsets": True, "use_regex": True}, decoder={"type": "ByteLevel", "add_prefix_space": True, "trim_offsets": True, "use_regex": True}))
show("replace regex", tok(bpe(), normalizer={"type": "Replace", "pattern": {"Regex": "\\s+"}, "content": " "}, pre=ws))
wp = {"type": "WordPiece", "unk_token": "<unk>", "continuing_subword_prefix": "##", "max_input_chars_per_word": 8, "vocab": IDS}
bert_post = {"type": "BertProcessing", "sep": ["[SEP]", IDS["[SEP]"]], "cls": ["[CLS]", IDS["[CLS]"]]}
bn = {"type": "BertNormalizer", "clean_text": True, "handle_chinese_chars": True, "strip_accents": None, "lowercase": True}
show("wordpiece bert", tok(wp, normalizer=bn, pre={"type": "BertPreTokenizer"}, post=bert_post, decoder={"type": "WordPiece", "prefix": "##", "cleanup": True},
                           added=[("[CLS]", True, {}), ("[SEP]", True, {}), ("[MASK]", True, {"lstrip": True})]),
     texts=TEXTS + ["the [MASK] is   [MASK]here"])
show("wordpiece pair", tok(wp, normalizer=bn, pre={"type": "BertPreTokenizer"}, post=bert_post), texts=TEXTS[:6], pair="and the end")
show("wordlevel", tok({"type": "WordLevel", "vocab": IDS, "unk_token": "<unk>"}, pre=ws, decoder={"type": "CTC", "pad_token": "[PAD]", "word_delimiter_token": "_", "cleanup": True}))
rob = {"type": "RobertaProcessing", "sep": ["</s>", IDS["</s>"]], "cls": ["<s>", IDS["<s>"]], "trim_offsets": True, "add_prefix_space": True}
show("roberta", tok(bpe(), pre={"type": "ByteLevel", "add_prefix_space": True, "trim_offsets": True, "use_regex": True}, post=rob, decoder={"type": "ByteLevel", "add_prefix_space": True, "trim_offsets": True, "use_regex": True}), texts=TEXTS[:6], pair=" the pair")
tpl = {"type": "TemplateProcessing",
       "single": [{"SpecialToken": {"id": "<s>", "type_id": 0}}, {"Sequence": {"id": "A", "type_id": 0}}, {"SpecialToken": {"id": "</s>", "type_id": 0}}],
       "pair": [{"SpecialToken": {"id": "<s>", "type_id": 0}}, {"Sequence": {"id": "A", "type_id": 0}}, {"SpecialToken": {"id": "</s>", "type_id": 0}},
                {"Sequence": {"id": "B", "type_id": 1}}, {"SpecialToken": {"id": "</s>", "type_id": 1}}],
       "special_tokens": {"<s>": {"id": "<s>", "ids": [IDS["<s>"]], "tokens": ["<s>"]}, "</s>": {"id": "</s>", "ids": [IDS["</s>"]], "tokens": ["</s>"]}}}
t = tok(bpe(), pre=ws, post={"type": "Sequence", "processors": [{"type": "ByteLevel", "add_prefix_space": True, "trim_offsets": False, "use_regex": True}, tpl]})
show("template", t, texts=TEXTS[:6], pair="the pair")
print(t.num_special_tokens_to_add(False), t.num_special_tokens_to_add(True))
for s in ["the ring", "the ring and more"]:
    e = t.encode(s, "and the end", add_special_tokens=False)
    print(e.ids, e.type_ids, e.sequence_ids)
show("added single_word", tok(bpe(), pre=ws, added=[("the", False, {"single_word": True}), ("ing", False, {"rstrip": True}), ("re", True, {"lstrip": True, "rstrip": True})]),
     texts=["the theme", "athe the", "ringing  the", "re re-enter  re  x"])
