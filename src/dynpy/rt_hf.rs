// ---------- Hugging Face tokenizers: the native module _brevi_hf (rt_hf.rs) ----------
// What src/dynpy/py/tokenizers.py calls: a tokenizer (rt_hfcore.rs's HTokenizer) as a V, its Encoding as
// tokenizers.Encoding, and the conversions pyo3 makes on the way in, with its messages. A tokenizer.json the
// program names by a literal (Tokenizer.from_file / from_pretrained) is built into the program by the
// compiler (src/dynpy/hf.rs) and found here by its key.

pub static HF_DATA: std::sync::OnceLock<Vec<(&'static str, &'static str)>> = std::sync::OnceLock::new();

fn hf_err(e: HfErr) -> E {
    match e {
        HfErr::Py(m) => exc("Exception", m),
        HfErr::Lacks(m) => unsupported(&m),
    }
}

pub struct HfCore(RefCell<HTokenizer>);

impl Ext for HfCore {
    fn tname(&self) -> &'static str {
        "tokenizers.Tokenizer"
    }
    fn repr(&self) -> String {
        "<tokenizers.Tokenizer object>".into()
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
}

pub struct HfEnc(HEnc);

fn hf_offs(o: &Offs) -> V {
    tup(vec![V::Int(o.0 as i64), V::Int(o.1 as i64)])
}
fn hf_u32s(x: &[u32]) -> V {
    list(x.iter().map(|i| V::Int(*i as i64)).collect())
}
fn hf_opt(x: Option<usize>) -> V {
    x.map_or(V::None, |i| V::Int(i as i64))
}

// pyo3's extraction of a usize / u32
fn hf_uint(v: &V, max: u64) -> Result<u64, E> {
    match v {
        V::Bool(b) => Ok(*b as u64),
        V::Int(i) if *i >= 0 && (*i as u64) <= max => Ok(*i as u64),
        V::Int(i) if *i < 0 && max == u64::MAX => Err(exc("OverflowError", "can't convert negative int to unsigned".into())),
        V::Int(_) => Err(exc("OverflowError", "out of range integral type conversion attempted".into())),
        V::Big(_) => Err(exc("OverflowError", if max == u64::MAX { "int too big to convert".into() } else { "Python int too large to convert to C long".into() })),
        _ => type_err(format!("'{}' object cannot be interpreted as an integer", tn(v))),
    }
}
fn hf_u32(v: &V) -> Result<u32, E> {
    hf_uint(v, u32::MAX as u64).map(|x| x as u32)
}
fn hf_usize(v: &V) -> Result<usize, E> {
    hf_uint(v, u64::MAX).map(|x| x as usize)
}
fn hf_ids(v: &V) -> Result<Vec<u32>, E> {
    match v {
        V::Str(_) => type_err("Can't extract `str` to `Vec`".into()),
        V::List(_) | V::Tuple(_) | V::Range(_) => to_vec(v)?.iter().map(hf_u32).collect(),
        _ => type_err(format!("'{}' object is not an instance of 'Sequence'", tn(v))),
    }
}
fn hf_bool(v: &V) -> Result<bool, E> {
    match v {
        V::Bool(b) => Ok(*b),
        _ => type_err(format!("'{}' object is not an instance of 'bool'", tn(v))),
    }
}

impl Ext for HfEnc {
    fn tname(&self) -> &'static str {
        "tokenizers.Encoding"
    }
    fn repr(&self) -> String {
        format!("Encoding(num_tokens={}, attributes=[ids, type_ids, tokens, offsets, attention_mask, special_tokens_mask, overflowing])", self.0.len())
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn len(&self) -> Option<usize> {
        Some(self.0.len())
    }
    fn attr(&self, _me: &V, n: &str) -> Option<R> {
        let e = &self.0;
        Some(Ok(match n {
            "ids" => hf_u32s(&e.ids),
            "type_ids" => hf_u32s(&e.type_ids),
            "tokens" => list(e.tokens.iter().map(|t| s(t)).collect()),
            "offsets" => list(e.offsets.iter().map(hf_offs).collect()),
            "attention_mask" => hf_u32s(&e.attention),
            "special_tokens_mask" => hf_u32s(&e.special),
            "word_ids" => list(e.words.iter().map(|w| w.map_or(V::None, |w| V::Int(w as i64))).collect()),
            "sequence_ids" => list(e.sequence_ids().into_iter().map(hf_opt).collect()),
            "n_sequences" => V::Int(e.n_sequences() as i64),
            "overflowing" => list(e.overflowing.iter().map(|o| V::Ext(Rc::new(HfEnc(o.clone())))).collect()),
            // deprecated (a DeprecationWarning), and the methods that change an Encoding
            "words" | "pad" | "truncate" | "set_sequence_id" | "merge" => return Some(Err(unsupported(&format!("tokenizers Encoding.{n}")))),
            _ => return None,
        }))
    }
    fn method(&self, _me: &V, n: &str, a: &mut Args) -> Option<R> {
        let e = &self.0;
        let names: &[&str] = match n {
            "word_to_tokens" | "word_to_chars" => &["word_index", "sequence_index"],
            "token_to_sequence" | "token_to_chars" | "token_to_word" => &["token_index"],
            "char_to_token" | "char_to_word" => &["char_pos", "sequence_index"],
            _ => return None,
        };
        Some((|| -> R {
            let mut args = vec![];
            for (i, k) in names.iter().enumerate() {
                let v = a.p.get(i).cloned().or_else(|| a.kw(k));
                args.push(v);
            }
            if a.p.len() > names.len() {
                return type_err(format!("Encoding.{n}() takes at most {} positional arguments ({} given)", names.len(), a.p.len()));
            }
            let Some(first) = args[0].clone() else {
                return type_err(format!("Encoding.{n}() missing 1 required positional argument: '{}'", names[0]));
            };
            let seq = match args.get(1).cloned().flatten() {
                Some(v) => hf_usize(&v)?,
                None => 0,
            };
            let pair = |p: Option<Offs>| p.map_or(V::None, |o| hf_offs(&o));
            Ok(match n {
                "word_to_tokens" => pair(e.word_to_tokens(hf_u32(&first)?, seq)),
                "word_to_chars" => pair(e.word_to_chars(hf_u32(&first)?, seq)),
                "token_to_sequence" => hf_opt(e.token_to_sequence(hf_usize(&first)?)),
                "token_to_chars" => pair(e.token_to_chars(hf_usize(&first)?)),
                "token_to_word" => e.token_to_word(hf_usize(&first)?).map_or(V::None, |w| V::Int(w as i64)),
                "char_to_token" => hf_opt(e.char_to_token(hf_usize(&first)?, seq)),
                _ => e.char_to_word(hf_usize(&first)?, seq).map_or(V::None, |w| V::Int(w as i64)),
            })
        })())
    }
}

fn hf_core_of(v: &V) -> Result<Rc<dyn Ext>, E> {
    match v {
        V::Ext(x) if x.any().downcast_ref::<HfCore>().is_some() => Ok(x.clone()),
        _ => type_err("expected a Tokenizer".into()),
    }
}

// TextInputSequence / PreTokenizedInputSequence
fn hf_input(v: &V, pretok: bool) -> Result<HInput, E> {
    if !pretok {
        return match v {
            V::Str(x) => Ok(HInput::Raw(x.s.to_string())),
            _ => type_err("TextInputSequence must be str".into()),
        };
    }
    match v {
        V::List(_) | V::Tuple(_) => {
            let mut ws = vec![];
            for x in to_vec(v)? {
                match &x {
                    V::Str(s) => ws.push(s.s.to_string()),
                    _ => return type_err("PreTokenizedInputSequence must be Union[List[str], Tuple[str]]".into()),
                }
            }
            Ok(HInput::Words(ws))
        }
        _ => type_err("PreTokenizedInputSequence must be Union[List[str], Tuple[str]]".into()),
    }
}

// one item of encode_batch: a sequence, or a pair as a tuple or a two-item list
fn hf_batch_item(v: &V, pretok: bool) -> Result<(HInput, Option<HInput>), E> {
    if let Ok(x) = hf_input(v, pretok) {
        return Ok((x, None));
    }
    if let V::Tuple(_) | V::List(_) = v {
        let xs = to_vec(v)?;
        if xs.len() == 2 {
            return Ok((hf_input(&xs[0], pretok)?, Some(hf_input(&xs[1], pretok)?)));
        }
    }
    type_err(if pretok {
        "PreTokenizedEncodeInput must be Union[PreTokenizedInputSequence, Tuple[PreTokenizedInputSequence, PreTokenizedInputSequence]]".into()
    } else {
        "TextEncodeInput must be Union[TextInputSequence, Tuple[InputSequence, InputSequence]]".into()
    })
}

fn hf_enc(e: HEnc) -> V {
    V::Ext(Rc::new(HfEnc(e)))
}

fn hf_new(t: HTokenizer) -> V {
    V::Ext(Rc::new(HfCore(RefCell::new(t))))
}

pub fn brevi_hf_fn(n: &str, a: Args) -> R {
    let arg = |i: usize| a.p.get(i).cloned().unwrap_or(V::None);
    if n == "load" {
        let j = arg(0);
        let V::Str(x) = &j else {
            return type_err(format!("'{}' object is not an instance of 'str'", tn(&j)));
        };
        return Ok(hf_new(hf_load(&x.s).map_err(hf_err)?));
    }
    if n == "load_built" {
        // a tokenizer.json the compiler built into the program, by key ("file:<path>", "hub:<id>@<revision>")
        let k = arg(0);
        let key = match &k {
            V::Str(x) => x.s.to_string(),
            _ => return type_err("expected a str".into()),
        };
        let text = HF_DATA.get().and_then(|d| d.iter().find(|(k, _)| *k == key)).map(|(_, t)| *t);
        let Some(text) = text else {
            return Err(unsupported(&format!("tokenizers {key} that was not found when the program was built")));
        };
        return Ok(hf_new(hf_load(text).map_err(hf_err)?));
    }
    if n == "lacks" {
        let w = arg(0);
        return Err(unsupported(&format!("tokenizers {}", if let V::Str(x) = &w { x.s.to_string() } else { "?".into() })));
    }
    let core = hf_core_of(&arg(0))?;
    let cell = &core.any().downcast_ref::<HfCore>().unwrap().0;
    match n {
        // encode(t, sequence, pair, is_pretokenized, add_special_tokens, fast)
        "encode" => {
            let pretok = truth(&arg(3))?;
            let seq = hf_input(&arg(1), pretok)?;
            let pair = match arg(2) {
                V::None => None,
                p => Some(hf_input(&p, pretok)?),
            };
            let add = hf_bool(&arg(4))?;
            let t = cell.borrow();
            Ok(hf_enc(t.encode_full(&seq, pair.as_ref(), add, truth(&arg(5))?).map_err(hf_err)?))
        }
        "encode_batch" => {
            let pretok = truth(&arg(2))?;
            let inp = arg(1);
            if let V::Str(_) = inp {
                return type_err("Can't extract `str` to `Vec`".into());
            }
            let items: Vec<(HInput, Option<HInput>)> = to_vec(&inp)?.iter().map(|x| hf_batch_item(x, pretok)).collect::<Result<_, _>>()?;
            let add = hf_bool(&arg(3))?;
            let t = cell.borrow();
            let out = t.encode_batch_full(&items, add, truth(&arg(4))?).map_err(hf_err)?;
            Ok(list(out.into_iter().map(hf_enc).collect()))
        }
        "decode" => {
            let ids = hf_ids(&arg(1))?;
            let skip = hf_bool(&arg(2))?;
            Ok(s(&cell.borrow().decode(&ids, skip).map_err(hf_err)?))
        }
        "decode_batch" => {
            let sv = arg(1);
            if let V::Str(_) = sv {
                return type_err("Can't extract `str` to `Vec`".into());
            }
            let seqs: Vec<Vec<u32>> = to_vec(&sv)?.iter().map(hf_ids).collect::<Result<_, _>>()?;
            let skip = hf_bool(&arg(2))?;
            let t = cell.borrow();
            let mut out = vec![];
            for x in &seqs {
                out.push(s(&t.decode(x, skip).map_err(hf_err)?));
            }
            Ok(list(out))
        }
        "token_to_id" => {
            let tv = arg(1);
            let V::Str(x) = &tv else {
                return type_err(format!("'{}' object is not an instance of 'str'", tn(&tv)));
            };
            Ok(cell.borrow().token_to_id(&x.s).map_or(V::None, |i| V::Int(i as i64)))
        }
        "id_to_token" => {
            let id = hf_u32(&arg(1))?;
            Ok(cell.borrow().id_to_token(id).map_or(V::None, |t| s(&t)))
        }
        "vocab" => {
            let mut v = cell.borrow().vocab(truth(&arg(1))?);
            v.sort_by_key(|x| x.1);
            let mut d = Dict::new();
            for (k, i) in v {
                d.set(s(&k), V::Int(i as i64))?;
            }
            Ok(V::Dict(Rc::new(RefCell::new(d))))
        }
        "vocab_size" => Ok(V::Int(cell.borrow().vocab_size(truth(&arg(1))?) as i64)),
        "n_added" => Ok(V::Int(cell.borrow().added_tokens(truth(&arg(1))?) as i64)),
        // set_truncation(t, None) or (t, max_length, stride, strategy, left)
        "set_truncation" => {
            let mut t = cell.borrow_mut();
            if let V::None = arg(1) {
                t.trunc = None;
                return Ok(V::None);
            }
            let max_length = hf_usize(&arg(1))?;
            let stride = hf_usize(&arg(2))?;
            let strategy = match &arg(3) {
                V::Str(x) if &*x.s == "longest_first" => "longest_first",
                V::Str(x) if &*x.s == "only_first" => "only_first",
                V::Str(x) if &*x.s == "only_second" => "only_second",
                V::Str(x) => return val_err(format!("Unknown `strategy`: `{}`. Use one of `longest_first`, `only_first`, or `only_second`", x.s)),
                v => return type_err(format!("'{}' object is not an instance of 'str'", tn(v))),
            };
            let left = match &arg(4) {
                V::Str(x) if &*x.s == "left" => true,
                V::Str(x) if &*x.s == "right" => false,
                V::Str(x) => return val_err(format!("Unknown `direction`: `{}`. Use one of `left` or `right`.", x.s)),
                v => return type_err(format!("'{}' object is not an instance of 'str'", tn(v))),
            };
            let n = t.added_tokens(false);
            let Some(eff) = max_length.checked_sub(n) else {
                return Err(unsupported("enable_truncation with max_length below the special tokens' count (tokenizers panics)"));
            };
            if eff < stride {
                return val_err(format!(
                    "tokenizer stride set to {stride}, which is greater than or equal to its effective max length of {eff} (= {max_length} original max length - {n} added special tokens), "
                ));
            }
            t.trunc = Some(HTrunc { max_length, stride, strategy, left });
            Ok(V::None)
        }
        "truncation" => {
            let t = cell.borrow();
            let Some(tr) = &t.trunc else {
                return Ok(V::None);
            };
            let mut d = Dict::new();
            d.set(s("max_length"), V::Int(tr.max_length as i64))?;
            d.set(s("stride"), V::Int(tr.stride as i64))?;
            d.set(s("strategy"), s(tr.strategy))?;
            d.set(s("direction"), s(if tr.left { "left" } else { "right" }))?;
            Ok(V::Dict(Rc::new(RefCell::new(d))))
        }
        // set_padding(t, None) or (t, left, pad_id, pad_type_id, pad_token, length, pad_to_multiple_of)
        "set_padding" => {
            let mut t = cell.borrow_mut();
            if let V::None = arg(1) {
                t.pad = None;
                return Ok(V::None);
            }
            let left = truth(&arg(1))?;
            let pad_id = hf_u32(&arg(2))?;
            let pad_type = hf_u32(&arg(3))?;
            let pv = arg(4);
            let V::Str(tok) = &pv else {
                return type_err(format!("'{}' object is not an instance of 'str'", tn(&pv)));
            };
            let fixed = match arg(5) {
                V::None => None,
                v => Some(hf_usize(&v)?),
            };
            let multiple = match arg(6) {
                V::None => None,
                v => Some(hf_usize(&v)?),
            };
            t.pad = Some(HPad { fixed, left, multiple, pad_id, pad_type, pad_token: tok.s.to_string() });
            Ok(V::None)
        }
        "padding" => {
            let t = cell.borrow();
            let Some(p) = &t.pad else {
                return Ok(V::None);
            };
            let mut d = Dict::new();
            d.set(s("length"), hf_opt(p.fixed))?;
            d.set(s("pad_to_multiple_of"), hf_opt(p.multiple))?;
            d.set(s("pad_id"), V::Int(p.pad_id as i64))?;
            d.set(s("pad_token"), s(&p.pad_token))?;
            d.set(s("pad_type_id"), V::Int(p.pad_type as i64))?;
            d.set(s("direction"), s(if p.left { "left" } else { "right" }))?;
            Ok(V::Dict(Rc::new(RefCell::new(d))))
        }
        // add_tokens(t, [(content, single_word, lstrip, rstrip, normalized, special), ...])
        "add_tokens" => {
            let mut toks = vec![];
            for x in to_vec(&arg(1))? {
                let f = to_vec(&x)?;
                let V::Str(c) = &f[0] else {
                    return type_err("Input must be a List[Union[str, AddedToken]]".into());
                };
                toks.push(HAddedTok {
                    content: c.s.to_string(),
                    single_word: truth(&f[1])?,
                    lstrip: truth(&f[2])?,
                    rstrip: truth(&f[3])?,
                    normalized: truth(&f[4])?,
                    special: truth(&f[5])?,
                });
            }
            let mut t = cell.borrow_mut();
            let t = &mut *t;
            let mut added = std::mem::take(&mut t.added);
            let n = added.add(toks, &t.model, t.norm.as_ref());
            t.added = added;
            Ok(V::Int(n as i64))
        }
        "set_encode_special" => {
            cell.borrow_mut().added.encode_special = truth(&arg(1))?;
            Ok(V::None)
        }
        "encode_special" => Ok(V::Bool(cell.borrow().added.encode_special)),
        // added_tokens_decoder(t): [(id, content, single_word, lstrip, rstrip, normalized, special)] by id
        "added_tokens" => {
            let t = cell.borrow();
            Ok(list(
                t.added
                    .map_r
                    .iter()
                    .map(|(id, x)| tup(vec![V::Int(*id as i64), s(&x.content), V::Bool(x.single_word), V::Bool(x.lstrip), V::Bool(x.rstrip), V::Bool(x.normalized), V::Bool(x.special)]))
                    .collect(),
            ))
        }
        _ => Err(unsupported(&format!("_brevi_hf.{n}"))),
    }
}
