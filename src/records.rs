// Python dicts used as records (`tx = {'n': n, 'date': d, 'posts': [], 'bad': False}`) hold values of
// different types under fixed keys. Each such shape becomes a record type, as a class would: the literal
// calls its constructor, and `tx['posts']` on a value of that type reads the field (see infer.rs).
// Records print as Python prints the dict and turn into JSON objects for json.dumps.
use crate::ast::*;
use std::collections::HashSet;

pub const PREFIX: &str = "AxRec";

// Rust and Brevi words that cannot be field names.
const RESERVED: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "test", "var", "self_",
];

fn key_name(k: &Expr) -> Option<String> {
    let Expr::Str(parts) = k else { return None };
    let [StrPart::Lit(s)] = parts.as_slice() else { return None };
    let ok = s.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    ok.then(|| field_of(s))
}

// The field that holds a key: a key that is a Rust or Brevi word (`'type'`) gets a trailing `_`.
pub fn field_of(key: &str) -> String {
    if RESERVED.contains(&key) { format!("{key}_") } else { key.to_string() }
}

// The dict key a record field stands for (as printed and in JSON).
pub fn key_of(field: &str) -> &str {
    match field.strip_suffix('_') {
        Some(k) if RESERVED.contains(&k) => k,
        _ => field,
    }
}

// What a value written in the literal plainly is, when that shows.
fn kind(e: &Expr) -> Option<&'static str> {
    match e {
        Expr::Int(_) | Expr::Float(_) => Some("num"),
        Expr::Unary("-", x) if matches!(x.as_ref(), Expr::Int(_) | Expr::Float(_)) => Some("num"),
        Expr::Str(_) => Some("str"),
        Expr::Bool(_) => Some("bool"),
        Expr::List(_) => Some("list"),
        Expr::Method(l, m, _, _) if m == "repeat" && matches!(l.as_ref(), Expr::List(_)) => Some("list"),
        Expr::Map(_) => Some("map"),
        Expr::Tuple(_) => Some("tuple"),
        Expr::Method(_, m, _, _) if m == "dec" || m == "frac" || m == "frac_div" => Some("dec"),
        _ => None,
    }
}

struct Shape {
    keys: Vec<String>,
    kinds: Vec<HashSet<&'static str>>,
    // The value every literal gives the key, when it is always the same empty one (`[]`, `{}`, None):
    // set in the constructor, so later lines decide its type as for a class's `self.tags = []`.
    fixed: Vec<Option<Expr>>,
    seen: Vec<bool>,
    line: usize,
    // The literal key sets that make this record.
    sets: Vec<Vec<String>>,
}

fn empty(e: &Expr) -> bool {
    // `[None] * 9`: a list of None slots, typed by what later lines put in them.
    let nones = |e: &Expr| matches!(e, Expr::Method(l, m, a, _) if m == "repeat" && matches!(a.as_slice(), [Expr::Int(_)]) && matches!(l.as_ref(), Expr::List(xs) if !xs.is_empty() && xs.iter().all(|x| matches!(x, Expr::Call(f, a, _) if f == "none" && a.is_empty()))));
    nones(e) || matches!(e, Expr::List(xs) if xs.is_empty()) || matches!(e, Expr::Map(xs) if xs.is_empty()) || matches!(e, Expr::Method(r, m, _, _) if m == "lax" && matches!(r.as_ref(), Expr::Map(xs) if xs.is_empty())) || matches!(e, Expr::Call(f, a, _) if f == "none" && a.is_empty())
}

fn shape_of(ps: &[(Expr, Expr)]) -> Option<Vec<String>> {
    if ps.is_empty() {
        return None;
    }
    let keys: Vec<String> = ps.iter().map(|(k, _)| key_name(k)).collect::<Option<_>>()?;
    let set: HashSet<&String> = keys.iter().collect();
    (set.len() == keys.len()).then_some(keys)
}

fn same(a: &[String], b: &[String]) -> bool {
    a.len() == b.len() && a.iter().all(|k| b.contains(k))
}

// The name a dict literal is put in (`cur = {...}`, `self.cur = {...}`), which ties its keys to the
// other literals put there.
fn holder(st: &Stmt) -> Option<(String, &Expr)> {
    match st {
        Stmt::Assign { target: Expr::Ident(n) | Expr::Field(_, n), op: "=", value, .. } => Some((n.clone(), value)),
        Stmt::Let { name: Pat::Bind(n), value, .. } => Some((n.clone(), value)),
        // A dict kept in a one-slot holder (`cur = [None]`, then `cur[0] = {"line": n, ...}`).
        Stmt::Assign { target: Expr::Index(b, k), op: "=", value: value @ Expr::Map(_), .. } if matches!(k.as_ref(), Expr::Int(_)) => match b.as_ref() {
            Expr::Ident(n) => Some((format!("@list {n}"), value)),
            _ => None,
        },
        // A dict kept under one key of another (`state['cur'] = {'bad': True}`, `state['cur'] = {...}`).
        Stmt::Assign { target: Expr::Index(b, k), op: "=", value: value @ Expr::Map(_), .. } if key_name(k).is_some() => match b.as_ref() {
            Expr::Ident(n) => Some((format!("@key {n} {}", key_name(k).unwrap()), value)),
            _ => None,
        },
        // Dicts pushed on one list (`stack.append({'kind': 'if', ...})`, `stack.append({'kind': 'while', ...})`)
        // are one record, as Python reads them alike.
        Stmt::Expr { expr: Expr::Method(r, m, args, _), .. } if matches!(m.as_str(), "push" | "append" | "insert" | "ax_push") => match (r.as_ref(), args.last()) {
            (Expr::Ident(n) | Expr::Field(_, n), Some(v @ Expr::Map(_))) => Some((format!("@list {n}"), v)),
            _ => None,
        },
        _ => None,
    }
}

pub fn records(items: &mut Vec<Item>, mixed: &HashSet<String>) {
    records_with(items, mixed, false)
}

// `eager`: also make a record of a dict literal whose values are all names, fields or calls (no plain
// literal shows their kinds), as Python code builds records from parameters. The caller retries
// without it when that does not type.
// Python lists of mixed literal kinds that are looped over (`vals = [1, 'a', True, None]`, `for v in vals`)
// are lists of JSON values, not fixed-slot rows: each item is wrapped as JSON.
pub fn json_lists(items: &mut [Item]) {
    fn kind(e: &Expr) -> Option<&'static str> {
        match e {
            Expr::Int(_) | Expr::Float(_) => Some("num"),
            Expr::Str(_) => Some("str"),
            Expr::Bool(_) => Some("bool"),
            Expr::List(_) => Some("list"),
            Expr::Map(_) => Some("map"),
            Expr::Call(n, a, _) if n == "none" && a.is_empty() => Some("none"),
            _ => None,
        }
    }
    fn mixed(xs: &[Expr]) -> bool {
        let ks: Option<HashSet<&str>> = xs.iter().map(kind).collect();
        xs.len() >= 2 && ks.is_some_and(|k| k.len() >= 2)
    }
    for it in items.iter_mut() {
        let (Item::Fn { body, .. } | Item::Test { body, .. }) = it else { continue };
        let mut looped: HashSet<String> = HashSet::new();
        crate::infer::walk(body, &mut |e| {
            if let Expr::For(_, it, _) = e {
                if let Expr::Ident(n) = it.as_ref() {
                    looped.insert(n.clone());
                }
            }
        });
        // A list of bools with None in it (`[True, None]`) is held as JSON too: a bool has no spare value for None.
        let bool_none = |xs: &[Expr]| xs.iter().any(|x| kind(x) == Some("none")) && xs.iter().all(|x| matches!(kind(x), Some("bool" | "none"))) && xs.iter().any(|x| kind(x) == Some("bool"));
        // A literal written out by json.dumps (`json.dumps({'a': [1, 2, {'b': None}], 'c': True})`): its
        // lists of mixed kinds hold JSON values.
        crate::parser::walk_mut(body, &mut |e| {
            if let Expr::Method(r, m, _, _) = e {
                if m == "py_dumps" && matches!(r.as_ref(), Expr::List(_) | Expr::Map(_)) {
                    // Every value inside becomes JSON, so no list or dict needs one item type.
                    let wrap = |v: Expr| if kind(&v) == Some("none") { Expr::Ident("Null".into()) } else { Expr::Call("ax_to_json".into(), vec![(None, v)], 0) };
                    crate::parser::walk_mut(r, &mut |x| match x {
                        Expr::List(xs) => {
                            for x in xs.iter_mut() {
                                let v = std::mem::replace(x, Expr::Int(0));
                                *x = wrap(v);
                            }
                        }
                        Expr::Map(ps) => {
                            for (_, v) in ps.iter_mut() {
                                let x = std::mem::replace(v, Expr::Int(0));
                                *v = wrap(x);
                            }
                        }
                        _ => {}
                    });
                }
            }
        });
        // A row pushed on a list (`self.students.append([name, {}])`): a name or value next to a fresh dict or
        // list is a fixed-slot row, read by index (`rows[i][1][q] = v`), so it is a tuple.
        crate::parser::walk_mut(body, &mut |e| {
            if let Expr::Method(_, m, args, _) = e {
                if matches!(m.as_str(), "push" | "append") && args.len() == 1 {
                    if let Expr::List(xs) = &args[0] {
                        let fresh = |x: &Expr| matches!(x, Expr::Map(p) if p.is_empty()) || matches!(x, Expr::List(p) if p.is_empty());
                        let plain = |x: &Expr| matches!(x, Expr::Ident(_) | Expr::Str(_) | Expr::Int(_) | Expr::Field(..));
                        if xs.len() >= 2 && xs.iter().any(fresh) && xs.iter().any(plain) && xs.iter().all(|x| fresh(x) || plain(x)) {
                            let xs = xs.clone();
                            args[0] = Expr::Tuple(xs);
                        }
                    }
                }
            }
        });
        crate::parser::walk_mut(body, &mut |e| {
            if let Expr::List(xs) = e {
                if bool_none(xs) {
                    for x in xs.iter_mut() {
                        let v = std::mem::replace(x, Expr::Int(0));
                        *x = if kind(&v) == Some("none") { Expr::Ident("Null".into()) } else { Expr::Call("ax_to_json".into(), vec![(None, v)], 0) };
                    }
                }
            }
        });
        if looped.is_empty() {
            continue;
        }
        crate::parser::walk_mut(body, &mut |e| {
            if let Expr::Block(stmts, _) = e {
                for st in stmts.iter_mut() {
                    if let Stmt::Let { name: Pat::Bind(n), value: Expr::List(xs), .. } | Stmt::Assign { target: Expr::Ident(n), op: "=", value: Expr::List(xs), .. } = st {
                        if looped.contains(n) && mixed(xs) {
                            for x in xs.iter_mut() {
                                let v = std::mem::replace(x, Expr::Int(0));
                                *x = if kind(&v) == Some("none") { Expr::Ident("Null".into()) } else { Expr::Call("ax_to_json".into(), vec![(None, v)], 0) };
                            }
                        }
                    }
                }
            }
        });
    }
}

pub fn records_with(items: &mut Vec<Item>, mixed: &HashSet<String>, eager: bool) {
    let py = crate::infer::py_on();
    for it in items.iter_mut() {
        if let Item::Fn { body, .. } | Item::Test { body, .. } = it {
            crate::parser::walk_mut(body, &mut |e| {
                if let Expr::Block(stmts, _) = e {
                    fold_keys(stmts);
                }
            });
        }
    }
    // Each distinct key set of a dict literal, and the names it is put in.
    let mut sets: Vec<(Vec<String>, HashSet<String>)> = vec![];
    let mut typed_mixed: Vec<Vec<String>> = vec![];
    let mut first_line = vec![];
    for it in items.iter() {
        let (fname, body, line) = match it {
            Item::Fn { name, body, line, .. } => (name.as_str(), body, line),
            Item::Test { body, line, .. } => ("", body, line),
            _ => continue,
        };
        crate::infer::walk(body, &mut |e| {
            if let Expr::Map(ps) = e {
                if let Some(keys) = shape_of(ps) {
                    if !sets.iter().any(|(k, _)| same(k, &keys)) {
                        sets.push((keys.clone(), HashSet::new()));
                        first_line.push(*line);
                    }
                    if mixed.contains(&crate::infer::map_key(ps)) {
                        typed_mixed.push(keys);
                    }
                }
            }
            if let Expr::Block(stmts, tail) = e {
                // (A branch's one statement is its tail: `stack.append({...})`.)
                let tail_st = tail.as_ref().map(|t| Stmt::Expr { expr: (**t).clone(), line: 0 });
                for st in stmts.iter().chain(tail_st.iter()) {
                    // Dicts a function returns in the same place (`return qid, {"type": "choice", ...}` and
                    // `return qid, {"type": "number", ...}`) are one record.
                    if let Stmt::Return { value: Some(v), .. } = st {
                        let slots: Vec<(usize, &Expr)> = match v {
                            Expr::Map(_) => vec![(0, v)],
                            Expr::Tuple(xs) => xs.iter().enumerate().filter(|(_, x)| matches!(x, Expr::Map(_))).collect(),
                            _ => vec![],
                        };
                        for (i, m) in slots {
                            let Expr::Map(ps) = m else { continue };
                            if let Some(keys) = shape_of(ps) {
                                let n = format!("@ret {fname} {i}");
                                if let Some(s) = sets.iter_mut().find(|(k, _)| same(k, &keys)) {
                                    s.1.insert(n);
                                } else {
                                    sets.push((keys, HashSet::from([n])));
                                    first_line.push(*line);
                                }
                            }
                        }
                    }
                    if let Some((n, Expr::Map(ps))) = holder(st) {
                        if let Some(keys) = shape_of(ps) {
                            if let Some(s) = sets.iter_mut().find(|(k, _)| same(k, &keys)) {
                                s.1.insert(n.to_string());
                            } else {
                                sets.push((keys, HashSet::from([n.to_string()])));
                                first_line.push(*line);
                            }
                        }
                    }
                }
            }
        });
    }
    // A name set from a call to a function that returns a record (`cur = new()`) holds that record,
    // so keys set on it later (`cur.setdefault("out", []).append(m)`) belong to it.
    let mut from_calls: Vec<(String, String)> = vec![];
    for it in items.iter() {
        let (Item::Fn { body, .. } | Item::Test { body, .. }) = it else { continue };
        crate::infer::walk(body, &mut |e| {
            if let Expr::Block(stmts, _) = e {
                for st in stmts {
                    match holder(st) {
                        Some((n, Expr::Call(f, _, _))) if !n.starts_with('@') => from_calls.push((n, format!("@ret {f} 0"))),
                        // `t = state['cur']`
                        Some((n, Expr::Index(h, k))) if !n.starts_with('@') && key_name(k).is_some() => {
                            if let Expr::Ident(h) = h.as_ref() {
                                from_calls.push((n, format!("@key {h} {}", key_name(k).unwrap())));
                            }
                        }
                        // `tx = cur[0]` from a holder of records.
                        Some((n, Expr::Index(h, k))) if !n.starts_with('@') && matches!(k.as_ref(), Expr::Int(_)) => {
                            if let Expr::Ident(h) = h.as_ref() {
                                from_calls.push((n, format!("@list {h}")));
                            }
                        }
                        _ => {}
                    }
                }
            }
        });
    }
    for (n, r) in from_calls {
        if let Some(s) = sets.iter_mut().find(|(_, names)| names.contains(&r)) {
            s.1.insert(n);
        }
    }
    // Keys set later on a name that holds a literal (`q = {...}`, then `q["answer"] = a` in a branch).
    let mut later: Vec<(String, String, Expr)> = vec![];
    for it in items.iter() {
        let (Item::Fn { body, .. } | Item::Test { body, .. }) = it else { continue };
        crate::infer::walk(body, &mut |e| {
            if let Expr::Block(stmts, _) = e {
                for st in stmts {
                    if let Stmt::Assign { target: Expr::Index(b, k), op: "=", value, .. } = st {
                        if let (Expr::Ident(n), Some(key)) = (b.as_ref(), key_name(k)) {
                            later.push((n.clone(), key, value.clone()));
                        }
                    }
                    // `cur.setdefault("msgs", []).append(m)` (read as `cur["msgs"].push(m)`): a list key set later.
                    if let Stmt::Expr { expr: Expr::Method(r, m, args, _), .. } = st {
                        if let Expr::Index(b, k) = r.as_ref() {
                            if let (Expr::Ident(n), Some(key)) = (b.as_ref(), key_name(k)) {
                                match (m.as_str(), args.as_slice()) {
                                    ("push" | "append" | "add", [a]) => later.push((n.clone(), key, Expr::List(vec![a.clone()]))),
                                    ("extend" | "update", [a]) => later.push((n.clone(), key, a.clone())),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        });
    }
    // Names used as a dict proper: indexed by a computed key, looped over, or asked for its keys.
    let mut dynamic: HashSet<String> = HashSet::new();
    for it in items.iter() {
        let (Item::Fn { body, .. } | Item::Test { body, .. }) = it else { continue };
        crate::infer::walk(body, &mut |e| match e {
            Expr::Index(b, k) if key_name(k).is_none() => {
                if let Expr::Ident(n) = b.as_ref() {
                    dynamic.insert(n.clone());
                }
            }
            Expr::Method(b, m, _, _) if matches!(m.as_str(), "pairs" | "keys" | "values" | "items" | "len" | "update" | "copy" | "pop" | "setdefault") => {
                if let Expr::Ident(n) = b.as_ref() {
                    dynamic.insert(n.clone());
                }
            }
            Expr::For(_, it, _) => {
                if let Expr::Ident(n) = it.as_ref() {
                    dynamic.insert(n.clone());
                }
            }
            _ => {}
        });
    }
    // Key sets put in the same name are one record (`cur = {'silent': True}` and the full one).
    let mut group: Vec<usize> = (0..sets.len()).collect();
    loop {
        let mut changed = false;
        for i in 0..sets.len() {
            for j in 0..i {
                if group[i] != group[j] && !sets[i].1.is_disjoint(&sets[j].1) {
                    let (a, b) = (group[i].min(group[j]), group[i].max(group[j]));
                    group.iter_mut().filter(|g| **g == b).for_each(|g| *g = a);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut shapes: Vec<Shape> = vec![];
    let mut none_vals: HashSet<String> = HashSet::new();
    let mut shape_of_set: Vec<usize> = vec![0; sets.len()];
    for g in 0..sets.len() {
        let members: Vec<usize> = (0..sets.len()).filter(|&i| group[i] == g).collect();
        if members.is_empty() {
            continue;
        }
        let mut keys: Vec<String> = vec![];
        for &m in &members {
            for k in &sets[m].0 {
                if !keys.contains(k) {
                    keys.push(k.clone());
                }
            }
        }
        // Later keys join only a record whose literals already show it is one (values of mixed kinds).
        let extra: Vec<String> = later.iter().filter(|(n, k, _)| members.iter().any(|&m| sets[m].1.contains(n)) && !keys.contains(k)).map(|(_, k, _)| k.clone()).collect();
        for k in extra {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
        for &m in &members {
            shape_of_set[m] = shapes.len();
        }
        shapes.push(Shape { kinds: vec![HashSet::new(); keys.len()], fixed: vec![None; keys.len()], seen: vec![false; keys.len()], keys, line: first_line[members[0]], sets: members.iter().map(|&m| sets[m].0.clone()).collect() });
    }
    for it in items.iter() {
        let (Item::Fn { body, .. } | Item::Test { body, .. }) = it else { continue };
        crate::infer::walk(body, &mut |e| {
            let Expr::Map(ps) = e else { return };
            let Some(keys) = shape_of(ps) else { return };
            let Some(si) = sets.iter().position(|(k, _)| same(k, &keys)) else { return };
            let sh = &mut shapes[shape_of_set[si]];
            for (k, v) in ps {
                let name = key_name(k).unwrap();
                if matches!(v, Expr::Call(n, a, _) if n == "none" && a.is_empty()) {
                    none_vals.insert(name.clone());
                }
                let j = sh.keys.iter().position(|x| *x == name).unwrap();
                if let Some(kd) = kind(v) {
                    sh.kinds[j].insert(kd);
                }
                let fix = if empty(v) && (!sh.seen[j] || sh.fixed[j].as_ref().is_some_and(|f| format!("{f:?}") == format!("{v:?}"))) { Some(v.clone()) } else { None };
                sh.fixed[j] = fix;
                sh.seen[j] = true;
            }
        });
    }
    for sh in shapes.iter_mut() {
        for (j, k) in sh.keys.clone().iter().enumerate() {
            if sh.sets.iter().all(|set| !set.contains(k)) {
                // No literal has it: the constructor sets it to None and later lines give its type.
                sh.fixed[j] = Some(Expr::Call("none".into(), vec![], 0));
                for (_, lk, v) in later.iter().filter(|(_, lk, _)| lk == k) {
                    let _ = lk;
                    if let Some(kd) = kind(v) {
                        sh.kinds[j].insert(kd);
                    }
                }
            }
        }
    }
    // A record: at least two keys, values that plainly differ in kind (`[]` next to `False`) or are known
    // to differ in type; and no key holding two kinds of value (that would be a dict used as a dict).
    let chosen: Vec<&Shape> = shapes
        .iter()
        .filter(|s| {
            let all: HashSet<&str> = s.kinds.iter().flatten().copied().collect();
            // A key that starts as None next to keys of plain kinds (`"last": None, "count": 0`) gets its own type later.
            let none_slot = s.keys.iter().zip(&s.fixed).any(|(k, f)| matches!(f, Some(Expr::Call(n, _, _)) if n == "none") && s.sets.iter().any(|set| set.contains(k)));
            // Every value an empty collection (`{"players": [], "names": set()}`) and the dict only ever read by
            // its literal keys: each key gets its own type from later lines.
            let holders: Vec<&String> = sets.iter().filter(|(k, _)| s.sets.iter().any(|m| same(k, m))).flat_map(|(_, h)| h.iter()).collect();
            let fixed_use = !holders.is_empty() && holders.iter().all(|h| !dynamic.contains(*h));
            let all_empty = s.fixed.iter().all(|f| matches!(f, Some(Expr::List(_) | Expr::Map(_)))) && fixed_use;
            let differ = all_empty || all.len() >= 2 || (none_slot && !all.is_empty()) || typed_mixed.iter().any(|k| s.sets.iter().any(|m| same(k, m)));
            // Keys set later may show kinds; the literals' own keys show none.
            // A literal with some keys of unknown kind (`{"Name": name, "N": n, "big": True}`) may hide a mix too;
            // the eager try falls back to a plain dict if the record doesn't type.
            let own_plain = s.keys.iter().zip(&s.kinds).all(|(k, kd)| kd.is_empty() || !s.sets.iter().any(|set| set.contains(k)))
                || s.keys.iter().zip(&s.kinds).any(|(k, kd)| kd.is_empty() && s.sets.iter().any(|set| set.contains(k)));
            let named = eager && own_plain && holders.iter().all(|h| !dynamic.contains(*h)) && s.keys.iter().zip(&s.fixed).all(|(k, f)| f.is_none() || !s.sets.iter().any(|set| set.contains(k)));
            // A key some literals leave out and another gives as None: absent and None would look alike.
            let is_none = |v: &Expr| matches!(v, Expr::Call(n, a, _) if n == "none" && a.is_empty());
            let ambiguous = py && s.keys.iter().any(|k| s.sets.iter().any(|set| !set.contains(k)) && s.sets.iter().any(|set| set.contains(k)) && (none_vals.contains(k) || later.iter().any(|(_, lk, v)| lk == k && is_none(v))));
            !ambiguous && s.keys.len() >= 2 && (differ || named) && s.kinds.iter().all(|k| k.len() <= 1 || k.iter().all(|x| *x == "num"))
        })
        .collect();
    if chosen.is_empty() {
        return;
    }
    // Numbers go on from records made by an earlier call.
    let base = items.iter().filter(|it| matches!(it, Item::Type { name, .. } if name.starts_with(PREFIX))).count();
    // Keys passed to the constructor (the others are fixed there).
    let names: Vec<(String, Vec<Vec<String>>, Vec<String>)> = chosen
        .iter()
        .enumerate()
        .map(|(n, s)| (format!("{PREFIX}{}", n + base), s.sets.clone(), s.keys.iter().zip(&s.fixed).filter(|(_, f)| f.is_none()).map(|(k, _)| k.clone()).collect()))
        .collect();
    for it in items.iter_mut() {
        let (Item::Fn { body, .. } | Item::Test { body, .. }) = it else { continue };
        crate::parser::walk_mut(body, &mut |e| {
            let Expr::Map(ps) = e else { return };
            let Some(keys) = shape_of(ps) else { return };
            let Some((name, _, order)) = names.iter().find(|(_, sets, _)| sets.iter().any(|k| same(k, &keys))) else { return };
            // A key this literal leaves out holds None (Python would fail on reading it).
            let args: Vec<(Option<String>, Expr)> = order
                .iter()
                .map(|k| {
                    let v = ps.iter().find(|(x, _)| key_name(x).as_deref() == Some(k.as_str())).map(|p| p.1.clone()).unwrap_or(Expr::Call("none".into(), vec![], 0));
                    (None, v)
                })
                .collect();
            *e = Expr::Call(format!("{name}_new"), args, 0);
        });
    }
    for ((name, _, _), s) in names.iter().zip(&chosen) {
        for k in &s.keys {
            // Python mode: a key only some literals have is left out where they lack it.
            if s.sets.iter().all(|set| !set.contains(k)) || (py && s.sets.iter().any(|set| !set.contains(k))) {
                OPTIONAL.with(|o| o.borrow_mut().insert((name.clone(), k.clone())));
            }
        }
    }
    for ((name, _, passed), s) in names.iter().zip(&chosen) {
        let keys = &s.keys;
        let q = || Ty::Named("?".into());
        let line = s.line;
        items.push(Item::Type { name: name.clone(), fields: keys.iter().map(|k| Field { name: k.clone(), ty: q(), default: None }).collect(), line });
        let me = Ty::Named(name.clone());
        let mut stmts = vec![Stmt::Let { name: Pat::Bind("self_".into()), ty: Some(me), value: Expr::Call("__default".into(), vec![], line), mutable: true, line }];
        for (k, f) in keys.iter().zip(&s.fixed) {
            let value = f.clone().unwrap_or_else(|| Expr::Ident(format!("ax_{k}")));
            stmts.push(Stmt::Assign { target: Expr::Field(Box::new(Expr::Ident("self_".into())), k.clone()), op: "=", value, line });
        }
        items.push(Item::Fn {
            name: format!("{name}_new"),
            params: passed.iter().map(|k| Param { name: format!("ax_{k}"), ty: q(), is_mut: false }).collect(),
            ret: Ty::Named(name.clone()),
            fallible: false,
            body: Expr::Block(stmts, Some(Box::new(Expr::Ident("self_".into())))),
            line,
            script: false,
        });
    }
}

// `cur = {"players": {}}` then `cur["res"] = []` right after: one literal with both keys, as Python
// ends up with the same dict.
fn fold_keys(stmts: &mut Vec<Stmt>) {
    // `st.update({"cur": "0", "acc": None})` with literal keys: one assignment per key, so a record can hold it.
    let mut j = 0;
    while j < stmts.len() {
        if let Stmt::Expr { expr: Expr::Method(r, m, args, _), line } = &stmts[j] {
            if m == "update" && matches!(r.as_ref(), Expr::Ident(_)) {
                if let [Expr::Map(ps)] = args.as_slice() {
                    // Not when a value reads the dict (`d.update({"a": d["b"], "b": d["a"]})` reads before any change).
                    if !ps.is_empty() && ps.iter().all(|(k, _)| key_name(k).is_some()) && !format!("{ps:?}").contains(&format!("{r:?}")) {
                        let (r, line) = (r.clone(), *line);
                        let sets: Vec<Stmt> = ps.iter().map(|(k, v)| Stmt::Assign { target: Expr::Index(r.clone(), Box::new(k.clone())), op: "=", value: v.clone(), line }).collect();
                        let n = sets.len();
                        stmts.splice(j..j + 1, sets);
                        j += n;
                        continue;
                    }
                }
            }
        }
        j += 1;
    }
    let mut i = 0;
    while i < stmts.len() {
        let n = match &stmts[i] {
            Stmt::Assign { target: Expr::Ident(n), op: "=", value: Expr::Map(ps), .. } | Stmt::Let { name: Pat::Bind(n), value: Expr::Map(ps), .. } if shape_of(ps).is_some() => n.clone(),
            _ => {
                i += 1;
                continue;
            }
        };
        while i + 1 < stmts.len() {
            let Stmt::Assign { target: Expr::Index(b, k), op: "=", value, .. } = &stmts[i + 1] else { break };
            if !matches!(b.as_ref(), Expr::Ident(x) if *x == n) || key_name(k).is_none() || format!("{value:?}").contains(&format!("{n:?}")) {
                break;
            }
            let (k, v) = (k.as_ref().clone(), value.clone());
            if let Stmt::Assign { value: Expr::Map(ps), .. } | Stmt::Let { value: Expr::Map(ps), .. } = &mut stmts[i] {
                if ps.iter().any(|(x, _)| format!("{x:?}") == format!("{k:?}")) {
                    break;
                }
                ps.push((k, v));
            }
            stmts.remove(i + 1);
        }
        i += 1;
    }
}

thread_local! {
    // (record, field) pairs no literal of the record has: set later (`q["answer"] = a` in one branch). `'k' in d` can't be answered from the type for these, and
    // printing leaves them out while they are None.
    static OPTIONAL: std::cell::RefCell<HashSet<(String, String)>> = std::cell::RefCell::new(HashSet::new());
}

pub fn is_optional(rec: &str, field: &str) -> bool {
    OPTIONAL.with(|o| o.borrow().contains(&(rec.to_string(), field.to_string())))
}

pub fn is_record(name: &str) -> bool {
    name.starts_with(PREFIX)
}
