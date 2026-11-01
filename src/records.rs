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
    let ok = s.chars().next().is_some_and(|c| c.is_ascii_lowercase() || c == '_') && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
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
        Expr::Map(_) => Some("map"),
        Expr::Tuple(_) => Some("tuple"),
        Expr::Method(_, m, _, _) if m == "dec" => Some("dec"),
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
    matches!(e, Expr::List(xs) if xs.is_empty()) || matches!(e, Expr::Map(xs) if xs.is_empty()) || matches!(e, Expr::Call(f, a, _) if f == "none" && a.is_empty())
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
fn holder(st: &Stmt) -> Option<(&str, &Expr)> {
    match st {
        Stmt::Assign { target: Expr::Ident(n) | Expr::Field(_, n), op: "=", value, .. } => Some((n, value)),
        Stmt::Let { name: Pat::Bind(n), value, .. } => Some((n, value)),
        _ => None,
    }
}

pub fn records(items: &mut Vec<Item>, mixed: &HashSet<String>) {
    records_with(items, mixed, false)
}

// `eager`: also make a record of a dict literal whose values are all names, fields or calls (no plain
// literal shows their kinds), as Python code builds records from parameters. The caller retries
// without it when that does not type.
pub fn records_with(items: &mut Vec<Item>, mixed: &HashSet<String>, eager: bool) {
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
        let (Item::Fn { body, line, .. } | Item::Test { body, line, .. }) = it else { continue };
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
            if let Expr::Block(stmts, _) = e {
                for st in stmts {
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
                }
            }
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
            let differ = all.len() >= 2 || typed_mixed.iter().any(|k| s.sets.iter().any(|m| same(k, m)));
            // Keys set later may show kinds; the literals' own keys show none.
            let own_plain = s.keys.iter().zip(&s.kinds).all(|(k, kd)| kd.is_empty() || !s.sets.iter().any(|set| set.contains(k)));
            let named = eager && own_plain && s.keys.iter().zip(&s.fixed).all(|(k, f)| f.is_none() || !s.sets.iter().any(|set| set.contains(k)));
            s.keys.len() >= 2 && (differ || named) && s.kinds.iter().all(|k| k.len() <= 1 || k.iter().all(|x| *x == "num"))
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
            if s.sets.iter().all(|set| !set.contains(k)) {
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
