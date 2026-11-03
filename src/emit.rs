// Turns Brevi items into one Rust source file. v0 ownership strategy: every value read is
// cloned and `mut` parameters become &mut, so generated code never hits borrow errors.
use crate::ast::*;
use crate::diag::Diag;
use std::collections::{HashMap, HashSet};

// The allocator comes first so that the inner attribute can lead the file.
pub const PRELUDE: &str = concat!("#![allow(warnings)]\n", include_str!("alloc_rt.rs"), include_str!("prelude.rs"));

const BUILTINS: &[&str] = &["__shared", "print", "read", "write", "args", "input", "stdin", "lines", "str", "fail", "json", "csv", "date", "abs", "min", "max", "heap", "none", "__global", "__try", "__err", "__err_raw", "__exc", "__exc_is", "__exc_obj", "__exc_get", "__default", "__zero", "__truediv", "__readline", "__try_read", "__eprint"];
const FALLIBLE_BUILTINS: &[&str] = &["read", "write", "json", "date"];
pub const METHODS: &[&str] = &[
    "abs", "add_days", "all", "any", "big", "day", "days", "dec", "digit", "or", "ceil", "chars", "contains", "count", "ends_with", "enumerate", "filter", "first",
    "find", "find_from", "float", "floor", "fmt", "fold", "ints", "is_alpha", "is_digit", "is_space", "slice", "slice_step", "get_or", "has", "index_of", "insert", "int", "is_empty", "join", "keys", "last",
    "len", "lines", "lower", "map", "max", "min", "pairs", "pop", "push", "remove", "repeat", "replace", "rev",
    "round", "skip", "sort", "sort_by", "split", "sqrt", "starts_with", "sum", "take", "trim", "upper", "values",
    "weekday", "words", "year", "zip", "month", "code", "char", "unique", "step", "json", "spec", "flatten", "enumerate_from", "is_none",
    "text", "list", "is_null", "bool", "re_find", "re_full", "re_all", "re_list", "partition", "re_sub", "re_split",
    "trim_of", "trim_end", "trim_end_of", "trim_start", "trim_start_of", "is_upper", "is_lower", "is_alnum",
    "heap_push", "heap_pop", "heapify", "heap_push_pop", "heap_replace", "pop_at", "move_to_end", "pop_item", "next_or", "next_or_empty", "quantize", "frac", "frac_div", "csv_rows", "csv_dicts", "re_match", "re_findall", "next_or", "next_or_empty", "tally", "tally_words", "to_dict", "clear", "splice", "most_common", "most_common_all", "count_sub", "zfill", "title", "center", "center_with", "capitalize", "exit_with", "ljust", "rjust", "ljust_with", "rjust_with", "split_n", "rsplit_n", "update", "delta_days", "month_range", "is_leap", "is_ascii", "isinstance", "type_kind", "lines_keep", "pop_or", "py_repr", "py_dumps", "group_runs", "count_of", "min_or", "max_or", "first_or", "re_start", "re_end", "re_span", "re_groups", "isqrt", "lax", "int_base", "next_or_none", "tuple_items",
    "m_log", "m_logb", "m_log2", "m_log10", "m_log1p", "m_exp", "m_expm1", "m_sin", "m_cos", "m_tan", "m_asin", "m_acos", "m_atan", "m_atan2", "m_sinh", "m_cosh", "m_tanh", "m_hypot", "m_fabs", "m_trunc", "m_degrees", "m_radians", "m_copysign", "m_fmod", "m_isnan", "m_isinf", "m_isfinite", "m_inf", "m_nan", "m_time", "m_perf_counter", "m_monotonic", "bin_text", "hex_text", "oct_text", "starts_with_at", "ends_with_at",
];

// Methods that return a new value; calling one as a statement does nothing.
const PURE_METHODS: &[&str] = &[
    "sort", "sort_by", "rev", "map", "filter", "take", "skip", "trim", "lower", "upper", "replace", "enumerate", "zip", "unique", "step", "slice_step", "flatten", "enumerate_from",
    "trim_of", "trim_end", "trim_end_of", "trim_start", "trim_start_of",
];
const MUTATING_METHODS: &[&str] = &["push", "pop_or", "pop", "insert", "remove", "clear", "splice", "update", "heap_push", "heap_pop", "heapify", "heap_push_pop", "heap_replace", "pop_at", "move_to_end", "pop_item", "next_or", "next_or_empty", "next_or_none"];
// Methods whose lambda may use `?`; the whole call then fails with the first error.
const TRY_METHODS: &[&str] = &["map", "filter", "any", "all", "count", "fold", "sort_by", "re_sub"];
// Methods whose lambda takes one item: `(a, b) => ...` there unpacks a tuple item.
const ONE_ARG_METHODS: &[&str] = &["map", "filter", "any", "all", "count", "sort_by"];

struct FnSig {
    params: Vec<Param>,
    fallible: bool,
    ret: Ty,
}

pub struct Emitter {
    // Locals set from set(...) / a - b on sets in this function, so `s - k` on them is set difference.
    set_vars: HashSet<String>,
    // Local lists kept as AxSet (a list with a hash index, so `x in s` is O(1)); see fast_sets.
    hash_sets: HashSet<String>,
    // Per function: the parameters taken as AxSet (callers wrap a list they pass there).
    set_params: HashMap<String, Vec<usize>>,
    // Fields (".name") kept as AxSet everywhere, and the function's own lists kept as AxQueue.
    set_fields: HashSet<String>,
    queues: HashSet<String>,
    // Crates, rust fn and py fn code (see ffi.rs).
    pub foreign: crate::ffi::Foreign,
    // Files the program uses: (name, first line in the combined source, line count).
    pub files: Vec<(String, usize, usize)>,
    structs: HashMap<String, Vec<Field>>,
    // Python classes, which carry a None mark (types the runtime builds itself, at line 0, do not).
    marked: HashSet<String>,
    variants: HashMap<String, (String, Vec<Field>)>,
    fns: HashMap<String, FnSig>,
    // Per-function state.
    locals: HashSet<String>,
    refs: HashSet<String>, // names bound to &mut (mut params, iter_mut loop vars)
    // Script-level variables that functions use: name -> (type, line of its let).
    globals: HashMap<String, (Ty, usize)>,
    // Types of script-level names set so far, used or not, so `b = a + "x"` is known from `a`.
    top_tys: HashMap<String, Ty>,
    // Names first set inside a block that has ended, for a clearer "unknown name" error.
    block_only: HashSet<String>,
    fallible: bool,
    // The current function gives no value, so `return x` only leaves it.
    unit_fn: bool,
    // Inside a lambda that cannot return an error: a failing conversion stops the program.
    must: bool,
    // Python mode (the program uses def, class or global): values print and dicts order as in Python.
    pub py_mode: bool,
    // Labels of the try blocks being emitted: a failure inside one jumps to its handler.
    catch: Vec<String>,
    try_n: usize,
    // Open loops: (label, how many try blocks were open at the loop, whether a break/continue used the label).
    loops: Vec<(String, usize, bool)>,
    line: usize,
    pub diags: Vec<Diag>,
    // Names whose items are lists changed in place (`m[k].push(x)`), so an empty `m = {}` gets a partial type.
    nested: HashSet<String>,
    cur_body: String,
    json_ret: bool,
    // Top-level names that the current function sets before reading, so they are its own locals.
    own_locals: HashSet<String>,
    // Types for untyped empty values, by (name, line): (Rust type, complete).
    hints: HashMap<(String, usize), (String, bool)>,
    // Debug text of every function and test body, by name, for finding which ones share a top-level name.
    fn_texts: Vec<(String, String)>,
    // Fields that hold their own type (directly or in a cycle) are boxed: (owner type, field), and the field names.
    boxed: HashSet<(String, String)>,
    boxed_names: HashSet<String>,
    // Field names only Python objects (shared handles) have: read and written through the handle's data.
    shared_fields: HashSet<String>,
    // Keys of map literals that are Json objects (values of different types).
    mixed: HashSet<String>,
    // Names bound to boxed fields in the pattern being emitted, unboxed at the start of the arm.
    unbox: Vec<String>,
    // Borrow lists in loops instead of copying them (off when that breaks the borrow rules).
    pub borrow_loops: bool,
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == *cb { 0 } else { 1 };
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

pub(crate) fn closest<'a>(name: &str, options: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    options
        .map(|o| (edit_distance(name, o), o))
        // Only a near miss: a looser match points at an unrelated name (`readline` -> `re_find`, `int` -> `input`).
        .filter(|(d, _)| *d <= if name.len() <= 3 { 1 } else { 3.min(name.len() / 2) })
        .min()
        .map(|(_, o)| o)
}

// `(a, b)` as `(mut a, mut b)` when every item is a plain name.
fn mut_binds(ps: &[Pat], p: &str) -> String {
    if ps.iter().all(|p| matches!(p, Pat::Bind(_) | Pat::Wild)) {
        let names: Vec<String> = ps.iter().map(|p| match p { Pat::Bind(n) if n != "_" => format!("mut {n}"), _ => "_".into() }).collect();
        format!("({})", names.join(", "))
    } else {
        p.to_string()
    }
}

pub fn rust_ty(t: &Ty) -> String {
    match t {
        Ty::Named(n) => match n.as_str() {
            "int" => "i64".into(),
            "f64" | "float" => "f64".into(),
            "bool" => "bool".into(),
            "str" => "String".into(),
            "dec" => "Dec".into(),
            "big" => "Big".into(),
            "date" => "Date".into(),
            "json" => "Json".into(),
            other => other.into(),
        },
        Ty::List(t) => format!("Vec<{}>", rust_ty(t)),
        Ty::Map(k, v) => format!("AxM<{}, {}>", rust_ty(k), rust_ty(v)),
        Ty::Tuple(ts) if crate::infer::is_row_ast(t) => format!("AxRow<({},)>", ts.iter().map(rust_ty).collect::<Vec<_>>().join(", ")),
        Ty::Tuple(ts) => format!("({})", ts.iter().map(rust_ty).collect::<Vec<_>>().join(", ")),
        Ty::Unit => "()".into(),
    }
}

fn rust_str_lit(s: &str) -> String {
    format!("{s:?}")
}

// Does this expression use `?` outside any nested lambda?
fn contains_try(e: &Expr) -> bool {
    walk_try(e, false)
}

// Can this function body fail: any `?` (lambdas included) or a call to fail()?
fn can_fail(e: &Expr) -> bool {
    walk_try(e, true)
}

fn walk_try(e: &Expr, deep: bool) -> bool {
    let contains_try = |x: &Expr| walk_try(x, deep);
    let any = |xs: &[Expr]| xs.iter().any(|x| walk_try(x, deep));
    match e {
        Expr::Try(_) => true,
        Expr::If(c, _, Some(h)) if is_try(c) => contains_try(h),
        Expr::Call(n, _, _) if deep && n == "fail" => true,
        Expr::Lambda(_, b) => deep && contains_try(b),
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Ident(_) | Expr::Move(_) => false,
        Expr::Str(parts) => parts.iter().any(|p| matches!(p, StrPart::Interp(x) if contains_try(x))),
        Expr::List(xs) | Expr::Tuple(xs) => any(xs),
        Expr::Map(ps) => ps.iter().any(|(k, v)| contains_try(k) || contains_try(v)),
        Expr::Unary(_, a) | Expr::Field(a, _) | Expr::TupleField(a, _) => contains_try(a),
        Expr::Binary(_, a, b) | Expr::Index(a, b) | Expr::Range(a, b) | Expr::While(a, b) => contains_try(a) || contains_try(b),
        Expr::Call(_, args, _) => args.iter().any(|(_, a)| contains_try(a)),
        Expr::Method(r, _, args, _) => contains_try(r) || any(args),
        Expr::If(c, t, f) => contains_try(c) || contains_try(t) || f.as_ref().map_or(false, |f| contains_try(f)),
        Expr::Match(s, arms) => contains_try(s) || arms.iter().any(|(_, b)| contains_try(b)),
        Expr::For(_, i, b) => contains_try(i) || contains_try(b),
        Expr::Block(stmts, tail) => {
            tail.as_ref().map_or(false, |t| contains_try(t))
                || stmts.iter().any(|s| match s {
                    Stmt::Let { value, .. } => contains_try(value),
                    Stmt::Assign { target, value, .. } => contains_try(target) || contains_try(value),
                    Stmt::Expr { expr, .. } | Stmt::Assert { cond: expr, .. } => contains_try(expr),
                    Stmt::Return { value, .. } => value.as_ref().map_or(false, |v| contains_try(v)),
                    _ => false,
                })
        }
    }
}

// Puts `call` in place of the subject of the first `match n:`, inside statements and nested blocks.
fn subject_into_stmt(st: &mut Stmt, n: &str, call: &mut Option<Expr>) {
    match st {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => subject_into(value, n, call),
        Stmt::Expr { expr, .. } => subject_into(expr, n, call),
        Stmt::Return { value: Some(v), .. } => subject_into(v, n, call),
        _ => {}
    }
}
fn subject_into(e: &mut Expr, n: &str, call: &mut Option<Expr>) {
    if call.is_none() {
        return;
    }
    match e {
        Expr::Match(subj, arms) => {
            if matches!(subj.as_ref(), Expr::Ident(m) | Expr::Move(m) if m == n) {
                **subj = call.take().unwrap();
                return;
            }
            arms.iter_mut().for_each(|(_, b)| subject_into(b, n, call));
        }
        Expr::Block(st, t) => {
            st.iter_mut().for_each(|s| subject_into_stmt(s, n, call));
            if let Some(t) = t {
                subject_into(t, n, call);
            }
        }
        Expr::If(_, a, b) => {
            subject_into(a, n, call);
            if let Some(b) = b {
                subject_into(b, n, call);
            }
        }
        Expr::For(_, _, b) | Expr::While(_, b) => subject_into(b, n, call),
        _ => {}
    }
}

// `?` is optional on calls: a call to a function that can fail passes its error up unless it is
// already handled (`?`, `.or(...)`, or a match on Ok/Err). Repeats until no more functions become fallible.
pub fn auto_try(items: &mut [Item]) {
    loop {
        let mut fallible: HashSet<String> = FALLIBLE_BUILTINS.iter().map(|s| s.to_string()).collect();
        for it in items.iter() {
            if let Item::Fn { name, fallible: f, body, .. } = it {
                if *f || can_fail(body) {
                    fallible.insert(name.clone());
                }
            }
        }
        let before = fallible.len();
        for it in items.iter_mut() {
            match it {
                Item::Fn { body, .. } | Item::Test { body, .. } => wrap_calls(body, &fallible),
                _ => {}
            }
        }
        // Counted as a set like `fallible`, so a function defined twice cannot keep the loop going.
        let mut now = fallible.clone();
        for it in items.iter() {
            if let Item::Fn { name, fallible: f, body, .. } = it {
                if *f || can_fail(body) {
                    now.insert(name.clone());
                }
            }
        }
        if now.len() <= before {
            return;
        }
    }
}

// Conversions that fail on bad text (`"x".int()`); on numbers they cannot fail.
const CONVERT_METHODS: &[&str] = &["int", "int_base", "float", "ints", "big", "dec", "min", "max"];

fn wrap_calls(e: &mut Expr, f: &HashSet<String>) {
    // Walks into a call or conversion whose error is handled here, without wrapping it.
    let args_of = |e: &mut Expr, f: &HashSet<String>| match e {
        Expr::Call(_, args, _) => args.iter_mut().for_each(|(_, a)| wrap_calls(a, f)),
        Expr::Method(r, m, args, _) if CONVERT_METHODS.contains(&m.as_str()) => {
            wrap_calls(r, f);
            args.iter_mut().for_each(|a| wrap_calls(a, f));
        }
        other => wrap_calls(other, f),
    };
    match e {
        Expr::Try(inner) => args_of(inner, f),
        Expr::Method(recv, m, args, _) if m == "or" => {
            args_of(recv, f);
            args.iter_mut().for_each(|a| wrap_calls(a, f));
        }
        Expr::Match(subj, arms) => {
            if arms.iter().any(|(p, _)| matches!(p, Pat::Variant(n, _) if n == "Ok" || n == "Err")) {
                args_of(subj, f);
            } else {
                wrap_calls(subj, f);
            }
            arms.iter_mut().for_each(|(_, b)| wrap_calls(b, f));
        }
        Expr::Call(n, args, _) => {
            args.iter_mut().for_each(|(_, a)| wrap_calls(a, f));
            if f.contains(n.as_str()) {
                let call = std::mem::replace(e, Expr::Bool(false));
                *e = Expr::Try(Box::new(call));
            }
        }
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Ident(_) | Expr::Move(_) => {}
        Expr::Str(parts) => parts.iter_mut().for_each(|p| {
            if let StrPart::Interp(x) = p {
                wrap_calls(x, f)
            }
        }),
        Expr::List(xs) | Expr::Tuple(xs) => xs.iter_mut().for_each(|x| wrap_calls(x, f)),
        Expr::Map(ps) => ps.iter_mut().for_each(|(k, v)| {
            wrap_calls(k, f);
            wrap_calls(v, f)
        }),
        Expr::Unary(_, a) | Expr::Field(a, _) | Expr::TupleField(a, _) | Expr::Lambda(_, a) => wrap_calls(a, f),
        Expr::Binary(_, a, b) | Expr::Index(a, b) | Expr::Range(a, b) | Expr::While(a, b) | Expr::For(_, a, b) => {
            wrap_calls(a, f);
            wrap_calls(b, f)
        }
        Expr::Method(r, m, args, _) => {
            wrap_calls(r, f);
            args.iter_mut().for_each(|a| wrap_calls(a, f));
            if CONVERT_METHODS.contains(&m.as_str()) {
                let call = std::mem::replace(e, Expr::Bool(false));
                *e = Expr::Try(Box::new(call));
            }
        }
        Expr::If(c, t, e2) => {
            wrap_calls(c, f);
            wrap_calls(t, f);
            if let Some(x) = e2 {
                wrap_calls(x, f)
            }
        }
        Expr::Block(stmts, tail) => {
            for i in 0..stmts.len() {
                // `r = json(t)` later matched with Ok/Err arms keeps its error to be matched.
                let matched = match &stmts[i] {
                    Stmt::Assign { target: Expr::Ident(n), op: "=", value: Expr::Call(..), .. } | Stmt::Let { name: Pat::Bind(n), value: Expr::Call(..), .. } => {
                        let rest = format!("{:?} {:?}", &stmts[i + 1..], tail);
                        rest.contains(&format!("Match(Ident({n:?}), [(Variant(\"Ok\"")) || rest.contains(&format!("Match(Ident({n:?}), [(Variant(\"Err\""))
                    }
                    _ => false,
                };
                let (left, right) = stmts.split_at_mut(i);
                let _ = left;
                let s = &mut right[0];
                if matched {
                    let n = match &right[0] {
                        Stmt::Assign { target: Expr::Ident(n), .. } | Stmt::Let { name: Pat::Bind(n), .. } => n.clone(),
                        _ => unreachable!(),
                    };
                    let id = format!("Ident({n:?})");
                    let uses = format!("{:?} {:?}", &right[1..], tail).matches(&id).count() + format!("{:?} {:?}", &right[1..], tail).matches(&format!("Move({n:?})")).count();
                    if uses == 1 {
                        // Its one use is that match: match on the call itself, so the error reaches the Err arm.
                        let call = match &mut right[0] {
                            Stmt::Assign { value, .. } | Stmt::Let { value, .. } => std::mem::replace(value, Expr::Bool(false)),
                            _ => unreachable!(),
                        };
                        let mut call = Some(call);
                        for st in right[1..].iter_mut() {
                            if call.is_none() {
                                break;
                            }
                            subject_into_stmt(st, &n, &mut call);
                        }
                        if let (Some(c), Some(t)) = (call.take(), tail.as_mut()) {
                            let mut c = Some(c);
                            subject_into(t, &n, &mut c);
                        }
                    }
                    continue;
                }
                match s {
                    Stmt::Let { value, .. } => wrap_calls(value, f),
                    Stmt::Assign { target, value, .. } => {
                        wrap_calls(target, f);
                        wrap_calls(value, f)
                    }
                    Stmt::Expr { expr, .. } | Stmt::Assert { cond: expr, .. } => wrap_calls(expr, f),
                    Stmt::Return { value: Some(v), .. } => wrap_calls(v, f),
                    _ => {}
                }
            }
            if let Some(t) = tail {
                wrap_calls(t, f)
            }
        }
    }
}

// The value in the first `n = value` statement of a body.
fn first_assign<'a>(e: &'a Expr, n: &str) -> Option<&'a Expr> {
    match e {
        Expr::Block(stmts, tail) => {
            for st in stmts {
                let found = match st {
                    Stmt::Assign { target: Expr::Ident(t), op: "=", value, .. } if t == n => Some(value),
                    Stmt::Expr { expr, .. } => first_assign(expr, n),
                    _ => None,
                };
                if found.is_some() {
                    return found;
                }
            }
            tail.as_deref().and_then(|t| first_assign(t, n))
        }
        Expr::If(_, t, f) => first_assign(t, n).or_else(|| f.as_deref().and_then(|f| first_assign(f, n))),
        Expr::For(_, _, b) | Expr::While(_, b) => first_assign(b, n),
        Expr::Match(_, arms) => arms.iter().find_map(|(_, b)| first_assign(b, n)),
        _ => None,
    }
}

// Names set with `=` in the branches of an if or match (not inside loops).
fn branch_sets(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Block(stmts, tail) => {
            for st in stmts {
                match st {
                    Stmt::Assign { target: Expr::Ident(t), op: "=", .. } if !out.contains(t) => out.push(t.clone()),
                    // `(a, b) = f()` in a branch sets a and b.
                    Stmt::Assign { target: Expr::Tuple(xs), op: "=", .. } => {
                        for x in xs {
                            if let Expr::Ident(t) = x {
                                if !out.contains(t) {
                                    out.push(t.clone());
                                }
                            }
                        }
                    }
                    Stmt::Expr { expr, .. } => branch_sets(expr, out),
                    _ => {}
                }
            }
            if let Some(t) = tail {
                branch_sets(t, out);
            }
        }
        Expr::If(_, t, f) => {
            branch_sets(t, out);
            if let Some(f) = f {
                branch_sets(f, out);
            }
        }
        Expr::Match(_, arms) => arms.iter().for_each(|(_, b)| branch_sets(b, out)),
        _ => {}
    }
}

// True when the first mention of `n` in a function body (its Debug text) is `n = ...`.
// A plain local of function f: set at the top of its body before any read, read afterwards, and f does not call
// itself (a recursive function may pass results through the shared name).
fn owns(f: &str, body: &Expr, text: &str, n: &str) -> bool {
    // Set before any read, at the top of the body or first inside a branch (`if c: n = line.slice(1)`), as Python makes it local.
    let top_set = matches!(body, Expr::Block(..));
    // A recursive function shares its names across calls, unless it returns the name (an accumulator).
    let returned = text.contains(&format!("Return {{ value: Some(Ident({n:?}))")) || matches!(body, Expr::Block(_, Some(t)) if matches!(t.as_ref(), Expr::Ident(t) if t == n));
    top_set && (returned || !text.contains(&format!("Call({f:?}"))) && own_use(text, n)
}

// A function's own use of a name: it sets it before any read, and reads it afterwards.
// (A function that only assigns a name is writing shared state.)
fn own_use(body: &str, n: &str) -> bool {
    let id = format!("Ident({n:?})");
    sets_first(body, n) && body.matches(&id).count() + body.matches(&format!("Move({n:?})")).count() > body.matches(&format!("target: {id}, op: \"=\"")).count()
}

// Do the later statements read n before setting it? Blocks inside if, match and loops are checked on their own:
// a block that sets n first does not need the outer value.
fn stmts_set_first(rest: &[Stmt], tail: Option<&Expr>, n: &str) -> bool {
    !reads_before_set(rest, tail, n)
}

fn reads_before_set(stmts: &[Stmt], tail: Option<&Expr>, n: &str) -> bool {
    let mentions = |t: &str| t.contains(&format!("Ident({n:?})")) || t.contains(&format!("Move({n:?})"));
    for s in stmts {
        if !mentions(&format!("{s:?}")) {
            continue;
        }
        match s {
            Stmt::Assign { target: Expr::Ident(x), op: "=", value, .. } if x == n => return mentions(&format!("{value:?}")),
            Stmt::Expr { expr, .. } => {
                if expr_reads_before_set(expr, n) {
                    return true;
                }
            }
            _ => return true,
        }
    }
    tail.map_or(false, |t| expr_reads_before_set(t, n))
}

fn expr_reads_before_set(e: &Expr, n: &str) -> bool {
    let mentions = |t: &str| t.contains(&format!("Ident({n:?})")) || t.contains(&format!("Move({n:?})"));
    let block = |b: &Expr| match b {
        Expr::Block(st, t) => reads_before_set(st, t.as_deref(), n),
        other => mentions(&format!("{other:?}")),
    };
    match e {
        Expr::If(c, a, b) => mentions(&format!("{c:?}")) || block(a) || b.as_deref().map_or(false, |b| match b {
            Expr::If(..) => expr_reads_before_set(b, n),
            b => block(b),
        }),
        Expr::For(_, it, body) => mentions(&format!("{it:?}")) || block(body),
        Expr::While(c, body) => mentions(&format!("{c:?}")) || block(body),
        Expr::Match(subj, arms) => mentions(&format!("{subj:?}")) || arms.iter().any(|(_, a)| block(a)),
        Expr::Block(..) => block(e),
        other => mentions(&format!("{other:?}")),
    }
}

pub(crate) fn sets_first(body: &str, n: &str) -> bool {
    let id = format!("Ident({n:?})");
    body.find(&id).map_or(false, |i| {
        // `n = max(n, x)` reads n while setting it, so it is not set first.
        let rest = &body[i + id.len()..];
        let value = &rest[..rest.find(", line: ").unwrap_or(rest.len())];
        body[..i].ends_with("Assign { target: ") && body[i..].starts_with(&format!("{id}, op: \"=\"")) && !value.contains(&id) && !value.contains(&format!("Move({n:?})"))
    })
}

// Names a function declares with `global a, b`.
fn global_names(body: &Expr) -> HashSet<String> {
    let mut out = HashSet::new();
    if let Expr::Block(stmts, _) = body {
        for st in stmts {
            if let Stmt::Expr { expr: Expr::Call(f, args, _), .. } = st {
                if f == "__global" {
                    out.extend(args.iter().filter_map(|(_, a)| match a {
                        Expr::Ident(n) => Some(n.clone()),
                        _ => None,
                    }));
                }
            }
        }
    }
    out
}

// Names used as `name[k].push(..)` (or pop, insert, remove) anywhere in a body.
fn nested_lists(body: &Expr) -> HashSet<String> {
    let text = format!("{body:?}");
    let mut out = HashSet::new();
    let pat = "Method(Index(Ident(\"";
    let mut from = 0;
    while let Some(i) = text[from..].find(pat) {
        let start = from + i + pat.len();
        from = start;
        let Some(q) = text[start..].find('"') else { break };
        let name = &text[start..start + q];
        // Skip past the Index(...) to the method name.
        let bytes = text.as_bytes();
        let (mut depth, mut in_str) = (0i32, false);
        let mut j = start - pat.len() + "Method(".len();
        while j < bytes.len() {
            let c = bytes[j];
            if in_str {
                if c == b'\\' {
                    j += 1;
                } else if c == b'"' {
                    in_str = false;
                }
            } else if c == b'"' {
                in_str = true;
            } else if c == b'(' || c == b'[' || c == b'{' {
                depth += 1;
            } else if c == b')' || c == b']' || c == b'}' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            j += 1;
        }
        let rest = &text[(j + 1).min(text.len())..];
        if ["push", "pop_or", "pop", "insert", "remove", "clear", "splice", "update", "heap_push", "heap_pop", "heapify", "heap_push_pop", "heap_replace", "pop_at", "move_to_end", "pop_item", "next_or", "next_or_empty", "next_or_none"].iter().any(|m| rest.starts_with(&format!(", \"{m}\""))) {
            out.insert(name.to_string());
        }
    }
    out
}

// The variable at the bottom of a place like a.b[i].c.
fn root_name(e: &Expr) -> Option<&str> {
    match e {
        Expr::Ident(n) | Expr::Move(n) => Some(n),
        Expr::Field(b, _) | Expr::TupleField(b, _) | Expr::Index(b, _) => root_name(b),
        _ => None,
    }
}

// True when the block sets `name` or calls a method that changes it, like `name.sort()`.
// `m[k]`, `m[k].f`, `xs[i].0.f`: a place inside a list or map.
fn through_index(e: &Expr) -> bool {
    match e {
        Expr::Index(..) => true,
        Expr::Field(b, _) | Expr::TupleField(b, _) => through_index(b),
        _ => false,
    }
}

// A value unpacked level by level to the shape of a nested tuple pattern (lists or tuples at each level).
fn unpack_levels(p: &Pat, v: &str, k: &mut usize) -> String {
    match p {
        Pat::Tuple(ps) => {
            let base = *k;
            *k += ps.len();
            let names: Vec<String> = (0..ps.len()).map(|i| format!("ax_u{}", base + i)).collect();
            let parts: Vec<String> = ps.iter().zip(&names).map(|(q, n)| unpack_levels(q, n, k)).collect();
            format!("{{ let ({},) = AxUnpack::ax_unpack({v}); ({},) }}", names.join(", "), parts.join(", "))
        }
        _ => v.to_string(),
    }
}

fn changes(e: &Expr, name: &str) -> bool {
    if assigns_to(e, name) {
        return true;
    }
    changes_in_place(e, name)
}

// `row[i] = v`, `p.x = v` or `row.append(v)`: the item itself changes (not the name rebound).
pub(crate) fn changes_in_place(e: &Expr, name: &str) -> bool {
    let d = format!("{e:?}");
    // `row[i] = v` or `p.x = v` changes the item itself.
    if d.contains(&format!("target: Index(Ident({name:?})")) || d.contains(&format!("target: Field(Ident({name:?})")) || d.contains(&format!("target: TupleField(Ident({name:?})")) {
        return true;
    }
    ["push", "pop_or", "pop", "insert", "remove", "sort", "sort_by", "reverse", "clear", "splice", "update", "extend", "heap_push", "heap_pop", "heapify", "heap_push_pop", "heap_replace", "pop_at", "move_to_end", "pop_item", "next_or", "next_or_empty", "next_or_none"].iter().any(|m| d.contains(&format!("Method(Ident({name:?}), {m:?},")))
}

pub fn assigns_to(e: &Expr, name: &str) -> bool {
    match e {
        Expr::Block(stmts, tail) => {
            stmts.iter().any(|s| match s {
                Stmt::Assign { target: Expr::Ident(n), .. } => n == name,
                Stmt::Expr { expr, .. } => assigns_to(expr, name),
                _ => false,
            }) || tail.as_ref().map_or(false, |t| assigns_to(t, name))
        }
        Expr::If(_, a, b) => assigns_to(a, name) || b.as_ref().map_or(false, |b| assigns_to(b, name)),
        Expr::For(_, _, b) | Expr::While(_, b) => assigns_to(b, name),
        Expr::Match(_, arms) => arms.iter().any(|(_, b)| assigns_to(b, name)),
        _ => false,
    }
}

impl Emitter {
    pub fn new(items: &[Item], hints: bool) -> Emitter {
        let mut e = Emitter {
            foreign: Default::default(),
            files: vec![],
            structs: HashMap::new(),
            marked: HashSet::new(),
            variants: HashMap::new(),
            fns: HashMap::new(),
            locals: HashSet::new(),
            refs: HashSet::new(),
            globals: HashMap::new(),
            top_tys: HashMap::new(),
            block_only: HashSet::new(),
            fallible: false,
            unit_fn: false,
            must: false,
            py_mode: false,
            catch: vec![],
            loops: vec![],
            try_n: 0,
            borrow_loops: false,
            nested: HashSet::new(),
            cur_body: String::new(),
            json_ret: false,
            own_locals: HashSet::new(),
            set_vars: HashSet::new(),
            hash_sets: HashSet::new(),
            set_params: HashMap::new(),
            set_fields: HashSet::new(),
            queues: HashSet::new(),
            fn_texts: items
                .iter()
                .filter_map(|i| match i {
                    Item::Fn { name, body, script: false, .. } => Some((name.clone(), format!("{body:?}"))),
                    Item::Test { name, body, .. } => Some((format!("test {name}"), format!("{body:?}"))),
                    _ => None,
                })
                .collect(),
            boxed: HashSet::new(),
            boxed_names: HashSet::new(),
            shared_fields: HashSet::new(),
            unbox: Vec::new(),
            hints: HashMap::new(),
            mixed: HashSet::new(),
            line: 0,
            diags: Vec::new(),
        };
        let one = |n: &str, t: &str| vec![Field { name: "v".into(), ty: Ty::Named(t.into()), default: None }].into_iter().filter(|_| !n.is_empty()).collect::<Vec<_>>();
        for (v, t) in [("Null", ""), ("Bool", "bool"), ("Num", "f64"), ("Str", "str"), ("Arr", "[Json]"), ("Obj", "{str: Json}")] {
            e.variants.insert(v.into(), ("Json".into(), one(t, t)));
        }
        for it in items {
            match it {
                Item::Type { name, fields, line } => {
                    e.structs.insert(name.clone(), fields.clone());
                    // Python objects and dicts made records are shared handles.
                    if crate::infer::py_on() && (*line > 0 || crate::records::is_record(name)) {
                        e.marked.insert(name.clone());
                    }
                }
                Item::Enum { name, variants, .. } => {
                    for v in variants {
                        e.variants.insert(v.name.clone(), (name.clone(), v.fields.clone()));
                    }
                }
                Item::Fn { name, params, fallible, line, body, ret, .. } => {
                    if e.fns.contains_key(name) {
                        e.diag(*line, &format!("function '{name}' is defined twice"), None);
                    }
                    // `!` is optional: a function that uses ? or fail() can fail.
                    let fallible = *fallible || can_fail(body);
                    e.fns.insert(name.clone(), FnSig { params: params.clone(), fallible, ret: ret.clone() });
                }
                Item::Test { .. } => {}
            }
        }
        e.shared_fields = e.structs.iter().filter(|(n, _)| e.marked.contains(*n)).flat_map(|(_, fs)| fs.iter().map(|f| f.name.clone())).collect();
        let plain_fields: HashSet<String> = e.structs.iter().filter(|(n, _)| !e.marked.contains(*n)).flat_map(|(_, fs)| fs.iter().map(|f| f.name.clone())).collect();
        e.shared_fields.retain(|f| !plain_fields.contains(f));
        let (h, mixed) = crate::infer::empty_types(items);
        e.mixed = mixed;
        if hints {
            e.hints = h;
        }
        e.find_boxed(items);
        e.find_globals(items);
        e
    }

    // A field whose type leads back to the type that holds it (Expr inside Expr) must be boxed in Rust.
    fn find_boxed(&mut self, items: &[Item]) {
        let mut fields: HashMap<String, Vec<(String, Ty)>> = HashMap::new();
        for it in items {
            match it {
                Item::Type { name, fields: fs, .. } => fields.entry(name.clone()).or_default().extend(fs.iter().map(|f| (f.name.clone(), f.ty.clone()))),
                Item::Enum { name, variants, .. } => {
                    for v in variants {
                        fields.entry(name.clone()).or_default().extend(v.fields.iter().map(|f| (f.name.clone(), f.ty.clone())));
                    }
                }
                _ => {}
            }
        }
        // Types reachable through plain (unboxed, non-list) fields.
        fn reaches(fields: &HashMap<String, Vec<(String, Ty)>>, from: &str, to: &str, seen: &mut HashSet<String>) -> bool {
            if !seen.insert(from.to_string()) {
                return false;
            }
            fields.get(from).map_or(false, |fs| fs.iter().any(|(_, t)| match t {
                Ty::Named(n) => n == to || reaches(fields, n, to, seen),
                Ty::Tuple(ts) => ts.iter().any(|t| matches!(t, Ty::Named(n) if n == to || reaches(fields, n, to, &mut seen.clone()))),
                _ => false,
            }))
        }
        for (owner, fs) in &fields {
            for (f, t) in fs {
                if let Ty::Named(n) = t {
                    if fields.contains_key(n) && (n == owner || reaches(&fields, n, owner, &mut HashSet::new())) {
                        self.boxed.insert((owner.clone(), f.clone()));
                        self.boxed_names.insert(f.clone());
                    }
                }
            }
        }
    }

    fn field_ty(&self, owner: &str, f: &Field) -> String {
        if self.boxed.contains(&(owner.to_string(), f.name.clone())) {
            format!("AxB<{}>", rust_ty(&f.ty))
        } else if self.set_fields.contains(&format!(".{}", f.name)) {
            format!("AxSet<{}", &rust_ty(&f.ty)[4..])
        } else {
            rust_ty(&f.ty)
        }
    }

    // A script-level `let`/`var` that a function or test mentions becomes a global.
    fn find_globals(&mut self, items: &[Item]) {
        let Some(Expr::Block(stmts, _)) = items.iter().find_map(|i| match i {
            Item::Fn { script: true, body, .. } => Some(body),
            _ => None,
        }) else {
            return;
        };
        // (parameter names, debug text of the body) for every function and test.
        let others: Vec<(Vec<String>, String, String, &Expr)> = items
            .iter()
            .filter_map(|i| match i {
                Item::Fn { script: false, body, params, name, .. } => Some((params.iter().map(|p| p.name.clone()).collect(), format!("{body:?}"), name.clone(), body)),
                Item::Test { body, .. } => Some((vec![], format!("{body:?}"), String::new(), body)),
                _ => None,
            })
            .collect();
        let decls: Vec<(String, Option<Ty>, Expr, usize)> = stmts
            .iter()
            .filter_map(|st| match st {
                Stmt::Let { name: Pat::Bind(n), ty, value, line, .. } => Some((n.clone(), ty.clone(), value.clone(), *line)),
                Stmt::Assign { target: Expr::Ident(n), op: "=", value, line } => Some((n.clone(), None, value.clone(), *line)),
                _ => None,
            })
            .collect();
        let gtys = crate::infer::global_types(items);
        for (n, ty, value, line) in &decls {
            let (n, ty, value, line) = (n, ty, value, line);
            if let Some(t) = ty.clone().or_else(|| self.infer_ty(value)) {
                self.top_tys.entry(n.clone()).or_insert(t);
            }
            {
                // Used by name, and not a parameter or a local of the same name there.
                let used = others.iter().any(|(ps, body, _, _)| {
                    body.contains(&format!("Ident({n:?})")) && !ps.contains(n) && !body.contains(&format!("Bind({n:?})")) && !body.contains(&format!("{n:?}]"))
                });
                if !used || self.globals.contains_key(n) {
                    continue;
                }
                let hinted = self.hints.get(&(n.clone(), *line)).filter(|h| h.1).map(|h| Ty::Named(h.0.clone()));
                let found = ty.clone().or(hinted).or_else(|| self.infer_ty(value)).or_else(|| gtys.get(n).cloned());
                // A name every function sets before reading is each function's own local.
                let id = format!("Ident({n:?})");
                if others.iter().filter(|(_, b, _, _)| b.contains(&id)).all(|(ps, text, f, body)| ps.contains(n) || owns(f, body, text, n)) {
                    continue;
                }
                match found {
                    Some(t) => {
                        self.globals.insert(n.clone(), (t, *line));
                    }
                    // No type known: functions that set the name before reading it have their own local, as in Python.
                    None if others.iter().filter(|(_, b, _, _)| b.contains(&id)).all(|(ps, _, _, body)| ps.contains(n) || !expr_reads_before_set(body, n)) => {}
                    None => {
                        let example = match value {
                            Expr::List(_) => "[int] = []",
                            Expr::Map(_) => "{str: int} = {}",
                            _ => "TYPE = ...",
                        };
                        self.diag(*line, &format!("{n} is used inside a function, so it needs a type"), Some(format!("write {n}: {example} with the real type")));
                        // Known by name from here on, so later uses are not reported too.
                        self.globals.insert(n.clone(), (Ty::Named("_".into()), *line));
                    }
                }
            }
        }
    }

    // What print shows for an argument: a Python set prints in braces ({1, 2}, or set() when empty).
    fn print_arg(&mut self, e: &Expr) -> String {
        let set = self.py_mode && match e {
            Expr::Method(_, m, _, _) => m == "unique",
            Expr::Ident(n) | Expr::Move(n) => self.set_vars.contains(n),
            Expr::Binary("-" | "|" | "&", ..) => self.is_set_expr(e),
            _ => false,
        };
        if set { format!("ax_show_set(&{})", self.expr(e)) } else { self.expr(e) }
    }

    fn is_set_expr(&self, e: &Expr) -> bool {
        match e {
            Expr::Method(_, m, _, _) => ["unique", "keys", "values", "set_sub"].contains(&m.as_str()),
            Expr::Ident(n) | Expr::Move(n) => self.set_vars.contains(n),
            Expr::Binary("-" | "|" | "&", a, b) => self.is_set_expr(a) || self.is_set_expr(b),
            _ => false,
        }
    }

    // The type of a simple initial value, for globals.
    fn infer_ty(&self, e: &Expr) -> Option<Ty> {
        let named = |n: &str| Some(Ty::Named(n.into()));
        match e {
            Expr::Int(_) => named("int"),
            Expr::Float(_) => named("f64"),
            Expr::Bool(_) => named("bool"),
            Expr::Str(_) => named("str"),
            Expr::Unary("!", _) => named("bool"),
            Expr::Unary(_, a) => self.infer_ty(a),
            Expr::Ident(n) => self.globals.get(n).map(|g| g.0.clone()).or_else(|| self.top_tys.get(n).cloned()),
            Expr::Binary(op, a, b) => match *op {
                "==" | "!=" | "<" | ">" | "<=" | ">=" | "and" | "or" | "in" => named("bool"),
                "+" | "-" | "*" | "%" => {
                    let (x, y) = (self.infer_ty(a), self.infer_ty(b));
                    let is_f = |t: &Option<Ty>| matches!(t, Some(Ty::Named(n)) if n == "f64");
                    if is_f(&x) || is_f(&y) { named("f64") } else { x.or(y) }
                }
                _ => None,
            },
            Expr::List(xs) if !xs.is_empty() => Some(Ty::List(Box::new(self.infer_ty(&xs[0])?))),
            Expr::Map(ps) if !ps.is_empty() => {
                // A map of functions: the value type is the first named function's pointer type.
                let fptr = ps.iter().find_map(|(_, v)| match v {
                    Expr::Ident(n) if !self.locals.contains(n) && self.global(n).is_none() => self.fn_ptr_ty(n),
                    _ => None,
                });
                if let Some(f) = fptr {
                    return Some(Ty::Map(Box::new(self.infer_ty(&ps[0].0)?), Box::new(Ty::Named(format!("AxFn<{f}>")))));
                }
                Some(Ty::Map(Box::new(self.infer_ty(&ps[0].0)?), Box::new(self.infer_ty(&ps[0].1)?)))
            }
            Expr::Tuple(xs) => Some(Ty::Tuple(xs.iter().map(|x| self.infer_ty(x)).collect::<Option<Vec<_>>>()?)),
            Expr::Call(n, _, _) => match n.as_str() {
                "lines" | "args" => Some(Ty::List(Box::new(Ty::Named("str".into())))),
                "stdin" | "input" | "str" | "__readline" | "__try_read" => named("str"),
                "csv" => Some(Ty::List(Box::new(Ty::List(Box::new(Ty::Named("str".into())))))),
                "json" => named("Json"),
                "date" => named("Date"),
                n if self.fns.get(n).map_or(false, |f| !matches!(f.ret, Ty::Unit)) => Some(self.fns[n].ret.clone()),
                n if self.structs.contains_key(n) => named(n),
                n => self.variants.get(n).map(|(en, _)| Ty::Named(en.clone())),
            },
            Expr::Try(inner) => match inner.as_ref() {
                Expr::Method(_, m, _, _) if m == "int" => named("int"),
                Expr::Method(_, m, _, _) if m == "float" => named("f64"),
                Expr::Method(_, m, _, _) if m == "dec" || m == "big" => named(m),
                Expr::Method(_, m, _, _) if m == "quantize" || m == "frac" || m == "frac_div" => named("dec"),
                Expr::Method(_, m, _, _) if m == "csv_rows" => Some(Ty::List(Box::new(Ty::List(Box::new(Ty::Named("str".into())))))),
                Expr::Method(_, m, _, _) if m == "csv_dicts" => Some(Ty::List(Box::new(Ty::Map(Box::new(Ty::Named("str".into())), Box::new(Ty::Named("str".into())))))),
                Expr::Method(_, m, _, _) if m == "ints" => Some(Ty::List(Box::new(Ty::Named("int".into())))),
                Expr::Call(n, _, _) if n == "read" => named("str"),
                Expr::Call(n, _, _) if n == "json" => named("Json"),
                Expr::Call(n, _, _) if n == "date" => named("Date"),
                Expr::Call(..) => self.infer_ty(inner),
                _ => None,
            },
            Expr::Method(_, m, _, _) => match m.as_str() {
                "len" | "count" | "find" | "find_from" | "index_of" | "digit" => named("int"),
                "words" | "lines" | "lines_keep" | "chars" | "split" | "split_n" | "rsplit_n" => Some(Ty::List(Box::new(Ty::Named("str".into())))),
                "trim" | "lower" | "upper" | "join" | "replace" | "text" | "trim_of" | "trim_end" | "trim_end_of" | "trim_start" | "trim_start_of" | "zfill" | "title" | "center" | "center_with" | "capitalize" | "ljust" | "rjust" | "ljust_with" | "rjust_with" => named("str"),
                "list" => Some(Ty::List(Box::new(Ty::Named("Json".into())))),
                "dec" | "big" if matches!(e, Expr::Method(r, ..) if matches!(r.as_ref(), Expr::Int(_) | Expr::Float(_))) => named(m),
                "float" if matches!(e, Expr::Method(r, ..) if matches!(r.as_ref(), Expr::Int(_))) => named("f64"),
                _ => None,
            },
            _ => None,
        }
    }

    // A map literal with string keys and values of different kinds becomes a JSON object.
    // A lambda with its parameter types written out from a fn pointer type like `fn(i64, i64) -> i64`.
    fn lambda_typed(&mut self, e: &Expr, fptr: &str) -> String {
        let Expr::Lambda(params, body) = e else { return self.expr(e) };
        let inner = fptr.strip_prefix("fn(").unwrap_or("");
        let (mut tys, mut depth, mut cur) = (Vec::new(), 0, String::new());
        for (i, c) in inner.char_indices() {
            match c {
                '<' | '(' | '[' => depth += 1,
                '>' if depth > 0 && !inner[..i].ends_with('-') => depth -= 1,
                ')' | ']' if depth > 0 => depth -= 1,
                ')' => break,
                ',' if depth == 0 => {
                    tys.push(std::mem::take(&mut cur).trim().to_string());
                    continue;
                }
                _ => {}
            }
            cur.push(c);
        }
        if !cur.trim().is_empty() {
            tys.push(cur.trim().to_string());
        }
        if tys.len() != params.len() {
            return self.expr(e);
        }
        let outer = self.locals.clone();
        for p in params {
            self.locals.insert(p.clone());
        }
        let saved = std::mem::replace(&mut self.must, true);
        let ps: Vec<String> = params.iter().zip(&tys).map(|(p, t)| format!("{p}: {t}")).collect();
        let out = format!("(|{}| {})", ps.join(", "), self.expr(body));
        self.must = saved;
        self.locals = outer;
        out
    }

    // `fn(i64, i64) -> i64` for a plain (non-failing) function used as a value.
    fn fn_ptr_ty(&self, n: &str) -> Option<String> {
        let sig = self.fns.get(n).filter(|f| !f.fallible && n != "main")?;
        let ps: Vec<String> = sig.params.iter().map(|p| rust_ty(&p.ty)).collect();
        let ret = if matches!(sig.ret, Ty::Unit) { String::new() } else { format!(" -> {}", rust_ty(&sig.ret)) };
        Some(format!("fn({}){ret}", ps.join(", ")))
    }

    fn mixed_map(&self, pairs: &[(Expr, Expr)]) -> bool {
        if !pairs.iter().all(|(k, _)| matches!(k, Expr::Str(_))) {
            return false;
        }
        if self.mixed.contains(&crate::infer::map_key(pairs)) {
            return true;
        }
        let kinds: Vec<String> = pairs
            .iter()
            .filter_map(|(_, v)| match v {
                Expr::Map(ps) if self.mixed_map(ps) => Some("Json".to_string()),
                Expr::Call(n, args, _) if n == "Obj" || n == "Arr" || (n == "Str" || n == "Num") && args.len() == 1 => Some("Json".to_string()),
                Expr::Method(_, m, _, _) if m == "json" => Some("Json".to_string()),
                v => self.infer_ty(v).map(|t| rust_ty(&t)),
            })
            .collect();
        kinds.iter().any(|k| k != &kinds[0])
    }

    // A condition: bool values as they are, anything else by its truth (`if xs:` means not empty).
    fn cond(&mut self, c: &Expr) -> String {
        let plain = match c {
            Expr::Bool(_) => true,
            Expr::Binary(op, ..) => ["==", "!=", "<", ">", "<=", ">=", "in", "~=", "and", "or"].contains(op),
            Expr::Unary(op, _) => *op == "!",
            Expr::Method(_, m, _, _) => ["has", "is_empty", "contains", "starts_with", "ends_with", "any", "all", "is_digit", "is_alpha", "is_null", "bool", "is_upper", "is_lower", "is_alnum", "is_ascii", "is_leap", "isinstance"].contains(&m.as_str()),
            e => self.infer_ty(e).map_or(false, |t| matches!(&t, Ty::Named(n) if n == "bool")),
        };
        // `while q:` reads q in place (no copy of the whole list per test).
        let e = match c {
            Expr::Ident(n) if !plain && self.locals.contains(n) && self.global(n).is_none() => self.place(c),
            _ => self.expr(c),
        };
        if plain { e } else { format!("ax_t(&{e})") }
    }

    fn global(&self, n: &str) -> Option<String> {
        if self.locals.contains(n) || !self.globals.contains_key(n) || self.own_locals.contains(n) {
            return None;
        }
        Some(format!("(*ax_gp(unsafe {{ std::ptr::addr_of_mut!(AXG_{n}) }}, {n:?}))"))
    }

    fn diag(&mut self, line: usize, msg: &str, fix: Option<String>) {
        let mut d = Diag::new("check", line, 1, msg);
        d.fix = fix;
        self.diags.push(d);
    }

    // The last error can be fixed by renaming `old` to `new` on its line.
    fn edit_last(&mut self, old: String, new: String) {
        if let Some(d) = self.diags.pop() {
            let core = |s: &str| s.trim_matches(|c| c == '.' || c == '(').to_string();
            let (a, b) = (core(&old), core(&new));
            let mut d = d.with_edit(&old, &new);
            // Short names are too close to each other to guess (`a` is one letter from `m`).
            let len = a.chars().count();
            let allowed = if len >= 6 { 2 } else if len >= 4 { 1 } else { 0 };
            d.sure = edit_distance(&a, &b) <= allowed || a.to_lowercase() == b.to_lowercase();
            self.diags.push(d);
        }
    }

    pub fn program(&mut self, items: &[Item]) -> String {
        // The regex engine is left out of programs that do not use it, to keep builds fast.
        let prelude = if format!("{items:?}").contains("\"re_") { PRELUDE } else { &PRELUDE[..PRELUDE.find("// @regex").unwrap_or(PRELUDE.len())] };
        let mut out = String::from(prelude);
        out.push('\n');
        if self.py_mode && FAST_SETS.load(std::sync::atomic::Ordering::Relaxed) {
            let mut fields = fast_set_fields(items);
            for it in items {
                if let Item::Type { fields: fs, .. } = it {
                    for f in fs {
                        if !rust_ty(&f.ty).starts_with("Vec<") || self.boxed.iter().any(|(_, b)| b == &f.name) {
                            fields.remove(&format!(".{}", f.name));
                        }
                    }
                }
            }
            self.set_fields = fields;
            for it in items {
                if let Item::Fn { name, params, body, script: false, .. } = it {
                    if crate::ffi::foreign_body(body).is_some() {
                        continue;
                    }
                    let good = fast_sets(body, &[]);
                    let at: Vec<usize> = (0..params.len()).filter(|&i| !params[i].is_mut && good.contains(&params[i].name) && rust_ty(&params[i].ty).starts_with("Vec<")).collect();
                    if !at.is_empty() {
                        self.set_params.insert(name.clone(), at);
                    }
                }
            }
        }
        let fs: Vec<String> = self.files.iter().map(|(f, a, n)| format!("({f:?}, {a}, {n})")).collect();
        out.push_str(&format!("static AX_FILES: &[(&str, usize, usize)] = &[{}];\n", fs.join(", ")));
        out.push_str(&self.foreign.rust_top);
        out.push('\n');
        let mut py_names: Vec<String> = vec![];
        let mut tests = String::new();
        let mut test_n = 0;
        let mut gs: Vec<_> = self.globals.iter().map(|(n, (t, _))| (n.clone(), rust_ty(t))).collect();
        gs.sort();
        for (n, t) in gs {
            out.push_str(&format!("static mut AXG_{n}: Option<{t}> = None;\n"));
        }
        for it in items {
            match it {
                Item::Type { name, fields, line } => {
                    // A Python object is shared, as in Python: the value is a handle to data that lives for the
                    // whole run, so copies of it (in lists, fields, other names) all see the same object.
                    let marked = self.marked.contains(name);
                    let data = if marked { format!("{name}_D") } else { name.clone() };
                    out.push_str(&format!("/*@L{line}*/#[derive(Debug, Clone, PartialEq, PartialOrd, Default)]\npub struct {data} {{\n"));
                    for f in fields {
                        out.push_str(&format!("    pub {}: {},\n", f.name, self.field_ty(name, f)));
                    }
                    if marked {
                        out.push_str(&format!("}}\n#[derive(Clone, Copy)]\npub struct {name}(*mut {data});\n"));
                        out.push_str(&format!("impl {name} {{ pub fn ax_new(d: {data}) -> Self {{ {name}(Box::into_raw(Box::new(d))) }} #[inline] pub fn ax_d(&self) -> &'static mut {data} {{ if self.0.is_null() {{ ax_none_attr() }} unsafe {{ &mut *self.0 }} }} }}\n"));
                        out.push_str(&format!("impl Default for {name} {{ fn default() -> Self {{ {name}(std::ptr::null_mut()) }} }}\n"));
                        if crate::records::is_record(name) {
                            // `dict(v)` / `v.copy()` on a dict record: a new dict with the same values.
                            out.push_str(&format!("impl {name} {{ pub fn ax_to_dict(&self) -> Self {{ Self::ax_new((**self).clone()) }} pub fn skip(&self, _n: i64) -> Self {{ self.ax_to_dict() }} }}\n"));
                        }
                        out.push_str(&format!("impl std::ops::Deref for {name} {{ type Target = {data}; fn deref(&self) -> &{data} {{ if self.0.is_null() {{ ax_none_attr() }} unsafe {{ &*self.0 }} }} }}\n"));
                        out.push_str(&format!("impl std::ops::DerefMut for {name} {{ fn deref_mut(&mut self) -> &mut {data} {{ if self.0.is_null() {{ ax_none_attr() }} unsafe {{ &mut *self.0 }} }} }}\n"));
                        // Equal when the same object, or (as before) when both hold equal data.
                        out.push_str(&format!("impl PartialEq for {name} {{ fn eq(&self, o: &Self) -> bool {{ self.0 == o.0 || (!self.0.is_null() && !o.0.is_null() && **self == **o) }} }}\n"));
                        out.push_str(&format!("impl PartialOrd for {name} {{ fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {{ if self.0.is_null() || o.0.is_null() {{ return self.0.is_null().partial_cmp(&o.0.is_null()).map(|c| c.reverse()); }} (**self).partial_cmp(&**o) }} }}\n"));
                        out.push_str(&format!("impl std::fmt::Debug for {name} {{ fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {{ if self.0.is_null() {{ write!(f, \"None\") }} else {{ (**self).fmt(f) }} }} }}\n"));
                        out.push_str(&format!("impl AxIsNoneHandle for {name} {{ fn ax_null(&self) -> bool {{ self.0.is_null() }} }}\n"));
                        out.push_str(&format!("impl AxShared for {name} {{}}\n"));
                        out.push_str(&format!("impl AxWhole for {name} {{}}\n"));
                        // The `}` that ends the struct in the other cases ends this empty impl.
                        out.push_str(&format!("impl {data} {{\n"));
                    }
                    // Python's `__str__` (or `__repr__`) is how the object prints.
                    let dunder = ["___str__", "___repr__"].iter().map(|d| format!("{name}{d}")).find(|f| self.fns.get(f).is_some_and(|s| s.params.len() == 1 && !s.fallible));
                    match dunder {
                        // A Python dict made a record prints as the dict, and is a JSON object for json.dumps.
                        None if crate::records::is_record(name) => {
                            // A key only some of the dicts have is left out while it is None, as Python's dict lacks it.
                            let when = |f: &Field| if crate::records::is_optional(name, &f.name) { format!("if !AxNone::ax_is_none(&self.{}) ", f.name) } else { String::new() };
                            let parts: Vec<String> = fields.iter().map(|f| format!("{2}{{ v.push(format!(\"'{1}': {{}}\", show_item(&self.{0}))); }}", f.name, crate::records::key_of(&f.name), when(f))).collect();
                            let pairs: Vec<String> = fields.iter().map(|f| format!("{2}{{ v.push((String::from(\"{1}\"), AxToJson::ax_json(&self.{0}))); }}", f.name, crate::records::key_of(&f.name), when(f))).collect();
                            out.push_str(&format!("}}\nimpl Show for {name} {{ fn show(&self) -> String {{ let mut v: Vec<String> = vec![]; {} format!(\"{{{{{{}}}}}}\", v.join(\", \")) }} }}\n", parts.join(" ")));
                            out.push_str(&format!("impl AxToJson for {name} {{ fn ax_json(&self) -> Json {{ let mut v = vec![]; {} ax_obj(v) }} }}\n", pairs.join(" ")));
                            // pybridge: the dict given to a Python library.
                            if crate::pybridge::active() {
                                let pys: Vec<String> = fields.iter().map(|f| format!("{2}{{ v.push((String::from(\"{1}\"), AxToPy::ax_to_py(&self.{0}))); }}", f.name, crate::records::key_of(&f.name), when(f))).collect();
                                out.push_str(&format!("impl AxToPy for {name} {{ fn ax_to_py(&self) -> AxPy {{ let mut v = vec![]; {} ax_pyb_dict(v) }} }}\n", pys.join(" ")));
                            }
                        }
                        dunder => {
                            // A dataclass prints as CPython's generated __repr__ does: `P(x=1, y=0)`.
                            let dc = crate::parser::dataclass(name).filter(|_| self.py_mode).map(|(py, shown)| {
                                let parts: Vec<String> = shown
                                    .iter()
                                    .filter(|n| fields.iter().any(|f| &f.name == *n))
                                    .map(|n| format!("format!(\"{}={{}}\", show_item(&self.{n}))", crate::lexer::py_ident(n)))
                                    .collect();
                                format!("format!(\"{py}({{}})\", [{}].join(\", \"))", parts.join(", "))
                            });
                            let fn_ok = |d: &str| Some(format!("{name}{d}")).filter(|f| self.fns.get(f).is_some_and(|s| s.params.len() == 1 && !s.fallible));
                            let repr_fn = fn_ok("___repr__").map(|f| format!("{f}(self.clone()).to_string()"));
                            let show = match (&dunder, &dc) {
                                (Some(f), _) => format!("{f}(self.clone()).to_string()"),
                                (None, Some(d)) => d.clone(),
                                // A Python object without __str__ or __repr__: CPython's default repr.
                                (None, None) if self.py_mode && marked => format!("format!(\"<__main__.{} object at {{:p}}>\", self.0)", crate::lexer::py_ident(name)),
                                (None, None) => "format!(\"{:?}\", self)".to_string(),
                            };
                            // With __str__, repr() (and lists of it) still use __repr__, or the dataclass's.
                            let repr = if fn_ok("___str__").is_some() { repr_fn.or(dc) } else { None };
                            let null = if marked { "if self.0.is_null() { return \"None\".into(); } " } else { "" };
                            let null_r = if marked { "if self.0.is_null() { return Some(\"None\".into()); } " } else { "" };
                            let repr = repr.map_or(String::new(), |r| format!(" fn show_repr(&self) -> Option<String> {{ {null_r}Some({r}) }}"));
                            out.push_str(&format!("}}\nimpl Show for {name} {{ fn show(&self) -> String {{ {null}{show} }}{repr} }}\n"));
                        }
                    }
                    // Python's None in a slot of this type: the zero value.
                    if marked {
                        out.push_str(&format!("impl AxNone for {name} {{ fn ax_none() -> Self {{ Default::default() }} fn ax_is_none(&self) -> bool {{ self.0.is_null() }} }}\n"));
                        out.push_str(&format!("impl AxFresh for {name} {{ fn ax_fresh() -> Self {{ {name}::ax_new(Default::default()) }} }}\n"));
                    } else {
                        out.push_str(&format!("impl AxFresh for {name} {{ fn ax_fresh() -> Self {{ Default::default() }} }}\n"));
                        out.push_str(&format!("impl AxNone for {name} {{ fn ax_none() -> Self {{ Default::default() }} }}\n"));
                    }
                    // `if obj:` is true for any object (so false only for None), unless the class says how.
                    let has = |d: &str| self.fns.get(&format!("{name}{d}")).is_some_and(|s| s.params.len() == 1 && !s.fallible);
                    let truth = if has("___bool__") {
                        format!("{name}___bool__(self.clone())")
                    } else if has("___len__") {
                        format!("{name}___len__(self.clone()) != 0")
                    } else {
                        "!AxNone::ax_is_none(self)".to_string()
                    };
                    out.push_str(&format!("impl AxTruth for {name} {{ fn ax_truth(&self) -> bool {{ {truth} }} }}\n"));
                }
                Item::Enum { name, variants, line } => {
                    out.push_str(&format!("/*@L{line}*/#[derive(Debug, Clone, PartialEq, PartialOrd)]\npub enum {name} {{\n"));
                    for v in variants {
                        if v.fields.is_empty() {
                            out.push_str(&format!("    {},\n", v.name));
                        } else {
                            let fs: Vec<String> = v.fields.iter().map(|f| format!("{}: {}", f.name, self.field_ty(name, f))).collect();
                            out.push_str(&format!("    {} {{ {} }},\n", v.name, fs.join(", ")));
                        }
                    }
                    out.push_str(&format!("}}\nimpl Show for {name} {{ fn show(&self) -> String {{ format!(\"{{:?}}\", self) }} }}\n"));
                    // Default is the first variant, so `m[k]` can create entries of this type.
                    if let Some(v) = variants.first() {
                        let fs: Vec<String> = v.fields.iter().map(|f| format!("{}: Default::default()", f.name)).collect();
                        let val = if fs.is_empty() { format!("{name}::{}", v.name) } else { format!("{name}::{} {{ {} }}", v.name, fs.join(", ")) };
                        out.push_str(&format!("impl Default for {name} {{ fn default() -> Self {{ {val} }} }}\n"));
                    }
                }
                Item::Fn { name, params, ret, body, line, .. } => {
                    let fallible = self.fns.get(name).map_or(false, |f| f.fallible);
                    if let Some((lang, code)) = crate::ffi::foreign_body(body) {
                        out.push_str(&crate::ffi::rust_fn(name, params, ret, fallible, lang, code, *line, py_names.len()));
                        if lang == "py" {
                            py_names.push(name.clone());
                        }
                        continue;
                    }
                    out.push_str(&self.function(name, params, ret, fallible, body, *line));
                }
                Item::Test { body, line, name } => {
                    test_n += 1;
                    self.begin_fn(&[], true);
                    self.nested = nested_lists(body);
                    let b = self.body(body, &Ty::Unit, true);
                    let py = if self.py_mode { "ax_py_mode();\n    " } else { "" };
                    let b = b.replacen("{\n", &format!("{{\n    {py}AX_FILES_REG.set(AX_FILES).ok();\n"), 1);
                    tests.push_str(&format!("/*@L{line}*/#[test] fn t{test_n}_{}() -> R<()> {b}\n", slug(name)));
                }
            }
        }
        if !py_names.is_empty() {
            out.push_str(&crate::ffi::py_support(&crate::ffi::py_driver(&self.foreign, &py_names), items));
        }
        match self.fns.get("main") {
            Some(sig) if sig.fallible => {
                let py = if self.py_mode { "    ax_py_mode();\n" } else { "" };
                out.push_str(&format!(
                    "fn main() {{\n    ax_init(AX_FILES);\n{py}    if let Err(e) = ax_main() {{\n        eprintln!(\"{{{{\\\"e\\\":\\\"runtime\\\",{{}},\\\"msg\\\":{{:?}}}}}}\", ax_at(AX_LINE.load(std::sync::atomic::Ordering::Relaxed)), ax_exc_msg(&e.0));\n        std::process::exit(1);\n    }}\n}}\n"
                ))
            }
            Some(_) => {}
            None => out.push_str("fn main() {}\n"),
        }
        if !tests.is_empty() {
            out.push_str("#[cfg(test)]\nmod ax_tests {\n    use super::*;\n");
            out.push_str(&tests);
            out.push_str("}\n");
        }
        out
    }

    // `m = {}` later used as `m[k].push(x)`: Rust needs to know the items are lists.
    fn empty_hint(&self, n: &str, line: usize, value: &Expr) -> String {
        // `d.get(k, {})` takes d's own value type (or JSON, from a JSON object).
        if matches!(value, Expr::Method(_, m, a, _) if m == "get_or" && matches!(a.get(1), Some(Expr::Map(x)) if x.is_empty()) || m == "get_or" && matches!(a.get(1), Some(Expr::List(x)) if x.is_empty())) {
            return String::new();
        }
        match self.hints.get(&(n.to_string(), line)) {
            Some((t, _)) => format!(": {t}"),
            None => self.nested_hint(n, value),
        }
    }

    // Is n set again in the function being written (`best = x` after `best = None`)?
    fn set_later(&self, n: &str) -> bool {
        self.cur_body.matches(&format!("target: Ident({n:?})")).count() > 1
    }

    fn nested_hint(&self, n: &str, value: &Expr) -> String {
        if !self.nested.contains(n) {
            return String::new();
        }
        match value {
            Expr::Map(ps) if ps.is_empty() => ": AxM<_, Vec<_>>".into(),
            Expr::Method(r, m, _, _) if m == "lax" && matches!(r.as_ref(), Expr::Map(ps) if ps.is_empty()) => ": AxM<_, Vec<_>>".into(),
            Expr::List(xs) if xs.is_empty() => ": Vec<Vec<_>>".into(),
            _ => String::new(),
        }
    }

    fn begin_fn(&mut self, params: &[Param], fallible: bool) {
        self.own_locals.clear();
        self.set_vars.clear();
        self.hash_sets.clear();
        self.queues.clear();
        self.locals = params.iter().map(|p| p.name.clone()).collect();
        self.refs = params.iter().filter(|p| p.is_mut).map(|p| p.name.clone()).collect();
        self.fallible = fallible;
    }

    fn function(&mut self, name: &str, params: &[Param], ret: &Ty, fallible: bool, body: &Expr, line: usize) -> String {
        self.begin_fn(params, fallible);
        self.nested = nested_lists(body);
        self.cur_body = format!("{body:?}");
        if self.py_mode && FAST_SETS.load(std::sync::atomic::Ordering::Relaxed) {
            self.hash_sets = fast_sets(body, params);
            self.queues = fast_queues(body, params);
            self.queues.retain(|n| !self.globals.contains_key(n));
            let q = self.queues.clone();
            self.hash_sets.retain(|n| !self.globals.contains_key(n) && !q.contains(n));
            for &i in self.set_params.get(name).map_or(&[][..], |v| v.as_slice()) {
                self.hash_sets.insert(params[i].name.clone());
            }
        }
        let set_at: Vec<usize> = self.set_params.get(name).cloned().unwrap_or_default();
        // A top-level name that this function first sets to a value of another type is the function's own.
        let text = format!("{body:?}");
        // A program with `global x` lines follows Python: setting a top-level name anywhere in a
        // function, without declaring it global there, makes it the function's own local.
        let py_scope = self.fn_texts.iter().any(|(_, t)| t.contains("Call(\"__global\"")) && self.fn_texts.iter().any(|(f, _)| f == name);
        if py_scope {
            let declared = global_names(body);
            let names: Vec<String> = self.globals.keys().filter(|n| !declared.contains(*n) && (assigns_to(body, n) || text.contains(&format!("For(Bind({n:?})")))).cloned().collect();
            self.own_locals.extend(names);
        }
        let names: Vec<String> = if py_scope { vec![] } else { self.globals.keys().filter(|n| sets_first(&text, n)).cloned().collect() };
        for n in names {
            // Set first here, and no other function reads it before setting it: a local that shares the name.
            let id = format!("Ident({n:?})");
            // Script code itself works on the top-level names.
            let script = !self.fn_texts.iter().any(|(f, _)| f == name);
            if !script && owns(name, body, &text, &n) && !self.fn_texts.iter().any(|(f, t)| f != name && t.contains(&id) && !own_use(t, &n) && !t.contains(&format!("Bind({n:?})"))) {
                self.own_locals.insert(n);
                continue;
            }
            let local_ty = first_assign(body, &n).and_then(|v| self.infer_ty(v));
            if let (Some(t), Some((g, _))) = (local_ty, self.globals.get(&n)) {
                if rust_ty(&t) != rust_ty(g) {
                    self.own_locals.insert(n);
                }
            }
        }
        self.line = line;
        self.unit_fn = matches!(ret, Ty::Unit);
        // A function giving JSON on some paths and a number on others: every value returned is made JSON.
        self.json_ret = matches!(ret, Ty::Named(n) if n == "json" || n == "Json");
        let ps: Vec<String> = params
            .iter()
            .enumerate()
            .map(|(i, p)| if p.is_mut { format!("{}: &mut {}", p.name, rust_ty(&p.ty)) } else if set_at.contains(&i) { format!("mut {}: AxSet<{}", p.name, &rust_ty(&p.ty)[4..]) } else { format!("mut {}: {}", p.name, rust_ty(&p.ty)) })
            .collect();
        let rname = if name == "main" && fallible { "ax_main" } else { name };
        let init = if name == "main" && !fallible { if self.py_mode { "ax_init(AX_FILES);\n    ax_py_mode();\n" } else { "ax_init(AX_FILES);\n" } } else { "" };
        if name == "main" && !params.is_empty() {
            self.diag(line, "main takes no parameters", Some("use args() to read command line arguments".into()));
        }
        let rret = if fallible { format!(" -> R<{}>", rust_ty(ret)) } else if matches!(ret, Ty::Unit) { String::new() } else { format!(" -> {}", rust_ty(ret)) };
        let b = self.body(body, ret, fallible);
        let marks: String = params.iter().filter_map(|p| self.rand_order_mark(&p.name)).map(|m| format!("{m}\n    ")).collect();
        let init = format!("{init}{marks}");
        let b = if init.is_empty() { b } else { b.replacen("{\n", &format!("{{\n    {init}"), 1) };
        format!("/*@L{line}*/fn {rname}({}){rret} {b}\n", ps.join(", "))
    }

    // A function body: the tail value becomes the return value, wrapped in Ok for fallible functions.
    fn body(&mut self, body: &Expr, ret: &Ty, fallible: bool) -> String {
        let unit = matches!(ret, Ty::Unit);
        let (stmts, tail): (&[Stmt], Option<&Expr>) = match body {
            Expr::Block(s, t) => (s, t.as_deref()),
            e => (&[], Some(e)),
        };
        let mut out = String::from("{\n");
        for (i, s) in stmts.iter().enumerate() {
            out.push_str(&self.hoist(s, &stmts[i + 1..], tail));
            out.push_str(&self.stmt(s));
        }
        if let Some(t) = tail {
            out.push_str(&self.loop_hoist(t));
        }
        match tail {
            Some(t) if unit => {
                out.push_str(&format!("    {};\n", self.stmt_expr(t)));
                if fallible {
                    out.push_str("    Ok(())\n");
                }
            }
            Some(t) => {
                let v = self.expr(t);
                out.push_str(&if fallible { format!("    Ok({v})\n") } else { format!("    {v}\n") });
            }
            None if fallible && unit => out.push_str("    Ok(())\n"),
            None if !unit => {
                // A non-unit function must end in a value unless every path returns.
                let ends_in_return = matches!(stmts.last(), Some(Stmt::Return { .. }));
                // `while True:` with no break of its own, that leaves only by return, is the end too.
                let forever = match stmts.last() {
                    Some(Stmt::Expr { expr: Expr::While(c, body), .. }) if matches!(c.as_ref(), Expr::Bool(true)) => !own_break(body),
                    _ => false,
                };
                if forever {
                    out.push_str("    #[allow(unreachable_code)] unreachable!()\n");
                } else if !ends_in_return {
                    self.diag(self.line, "function body ends without a value", Some("make the last line the value to return".into()));
                }
            }
            None => {}
        }
        out.push('}');
        out
    }

    fn loop_hoist(&mut self, e: &Expr) -> String {
        let mut out = String::new();
        // Python: a name set on some paths of a loop body keeps its value into the next pass
        // (`if key != prev: pos = i + 1` then `print(pos)`), so it is declared before the loop.
        if let Expr::For(_, _, b) | Expr::While(_, b) = e {
            let pat = match e {
                Expr::For(p, _, _) => format!("{p:?}"),
                _ => String::new(),
            };
            if let (true, Expr::Block(body, btail)) = (self.py_mode, b.as_ref()) {
                for (j, st) in body.iter().enumerate() {
                    let Stmt::Expr { expr: e @ (Expr::If(..) | Expr::Match(..)), .. } = st else { continue };
                    let mut names = Vec::new();
                    branch_sets(e, &mut names);
                    // Not the loop's own names, nor names each pass sets before the if.
                    names.retain(|n| {
                        !pat.contains(&format!("{n:?}"))
                            && !body[..j].iter().any(|s| matches!(s, Stmt::Assign { target: Expr::Ident(x), op: "=", .. } if x == n) || matches!(s, Stmt::Let { name: Pat::Bind(x), .. } if x == n))
                    });
                    let later = format!("{:?} {btail:?}", &body[j + 1..]);
                    for n in names.into_iter().filter(|n| n != "_") {
                        if !self.locals.contains(&n) && self.global(&n).is_none() && (later.contains(&format!("Ident({n:?})")) || later.contains(&format!("Move({n:?})"))) && !stmts_set_first(&body[j + 1..], btail.as_deref(), &n) {
                            out.push_str(&format!("    let mut {n} = Default::default();\n"));
                            self.locals.insert(n);
                        }
                    }
                }
            }
        }
        out
    }

    // A name first set inside an if or match and used after it is declared before it
    // (Rust checks that every path sets it).
    fn hoist(&mut self, s: &Stmt, rest: &[Stmt], tail: Option<&Expr>) -> String {
        let mut out = String::new();
        let rest_stmts = rest;
        let s = match s {
            // A try statement with finally is a block whose first statement is the try.
            Stmt::Expr { expr: Expr::Block(st, _), .. } if matches!(st.first(), Some(Stmt::Expr { expr: Expr::If(c, ..), .. }) if is_try(c)) => &st[0],
            s => s,
        };
        if let Stmt::Expr { expr, .. } = s {
            out.push_str(&self.loop_hoist(expr));
        }
        // Python: a name set in a loop body (`v = el; break`) is still there after the loop.
        let lp = match s {
            Stmt::Expr { expr: e @ (Expr::For(..) | Expr::While(..)), .. } if self.py_mode => Some(e),
            // A loop with an else part: `ax_broke = false; for ...; if !ax_broke {...}`.
            Stmt::Expr { expr: Expr::Block(st, _), .. } if self.py_mode && matches!(st.first(), Some(Stmt::Assign { target: Expr::Ident(f), .. } | Stmt::Let { name: Pat::Bind(f), .. }) if f.starts_with("ax_broke")) => st.iter().find_map(|x| match x {
                Stmt::Expr { expr: e @ (Expr::For(..) | Expr::While(..)), .. } => Some(e),
                _ => None,
            }),
            _ => None,
        };
        if let Some(Expr::For(_, _, b) | Expr::While(_, b)) = lp {
            let pat = match lp {
                Some(Expr::For(p, _, _)) => format!("{p:?}"),
                _ => String::new(),
            };
            let mut names = Vec::new();
            branch_sets(b, &mut names);
            // Names a later lambda (a comprehension) mentions: it may set its own.
            let mut in_lambda = String::new();
            let whole = Expr::Block(rest_stmts.to_vec(), tail.map(|t| Box::new(t.clone())));
            crate::infer::walk(&whole, &mut |e| {
                if let Expr::Lambda(..) = e {
                    in_lambda.push_str(&format!("{e:?}"));
                }
            });
            names.retain(|n| !in_lambda.contains(&format!("Ident({n:?})")));
            let rest = format!("{rest:?} {tail:?}");
            // Not a name a later loop or comprehension binds for itself (`for sku, (q, c) in ...`).
            for n in names.into_iter().filter(|n| n != "_" && !n.starts_with("ax_broke") && !pat.contains(&format!("{n:?}")) && !rest.contains(&format!("Bind({n:?})"))) {
                if !self.locals.contains(&n) && self.global(&n).is_none() && (rest.contains(&format!("Ident({n:?})")) || rest.contains(&format!("Move({n:?})"))) && !stmts_set_first(rest_stmts, tail, &n) {
                    out.push_str(&format!("    let mut {n} = Default::default();\n"));
                    self.locals.insert(n);
                }
            }
        }
        if let Stmt::Expr { expr: e @ (Expr::If(..) | Expr::Match(..)), .. } = s {
            let mut names = Vec::new();
            branch_sets(e, &mut names);
            let rest = format!("{rest:?} {tail:?}");
            for n in names.into_iter().filter(|n| n != "_") {
                if !self.locals.contains(&n) && self.global(&n).is_none() && (rest.contains(&format!("Ident({n:?})")) || rest.contains(&format!("Move({n:?})"))) && !stmts_set_first(rest_stmts, tail, &n) {
                    // A zero start value, as in Python a name set on only some paths can still be read after.
                    out.push_str(&format!("    let mut {n} = Default::default();\n"));
                    self.locals.insert(n);
                }
            }
        }
        out
    }

    fn block(&mut self, e: &Expr) -> String {
        self.block_d(e, false)
    }

    // discard: the block is used as a statement, so its last value is thrown away.
    fn block_d(&mut self, e: &Expr, discard: bool) -> String {
        match e {
            Expr::Block(stmts, tail) => {
                // Names declared in a block end with it.
                let outer = self.locals.clone();
                let mut out = String::from("{\n");
                for (i, s) in stmts.iter().enumerate() {
                    out.push_str(&self.hoist(s, &stmts[i + 1..], tail.as_deref()));
                    out.push_str(&self.stmt(s));
                }
                if let Some(t) = tail {
                    out.push_str(&self.loop_hoist(t));
                    if discard {
                        out.push_str(&format!("    {};\n", self.stmt_expr(t)));
                    } else {
                        out.push_str(&format!("    {}\n", self.expr(t)));
                    }
                }
                out.push('}');
                let inner = std::mem::replace(&mut self.locals, outer);
                self.block_only.extend(inner.into_iter().filter(|n| !self.locals.contains(n)));
                out
            }
            other if discard => format!("{{ {}; }}", self.stmt_expr(other)),
            other => format!("{{ {} }}", self.expr(other)),
        }
    }

    // A `break`/`continue` inside a try block (a labeled block in Rust) names its loop.
    fn loop_label(&mut self) -> String {
        let depth = self.catch.len();
        match self.loops.last_mut() {
            Some((lab, d, used)) if depth > *d => {
                *used = true;
                format!(" '{lab}")
            }
            _ => String::new(),
        }
    }
    fn open_loop(&mut self) {
        self.try_n += 1;
        self.loops.push((format!("ax_lp{}", self.try_n), self.catch.len(), false));
    }
    fn close_loop(&mut self) -> String {
        match self.loops.pop() {
            Some((lab, _, true)) => format!("'{lab}: "),
            _ => String::new(),
        }
    }

    // An expression whose value is not used: if/match branches need not agree on a type.
    fn stmt_expr(&mut self, e: &Expr) -> String {
        match e {
            Expr::If(c, t, Some(h)) if is_try(c) => {
                self.try_n += 1;
                let lab = format!("ax_try{}", self.try_n);
                self.catch.push(lab.clone());
                let body = self.block_d(t, true);
                self.catch.pop();
                let handler = self.block_d(h, true);
                format!("{{ let mut {lab}: Option<AxErr> = None; '{lab}: {body} if let Some(AxErr(ax_err)) = {lab} {{ let _ = &ax_err; {handler} }} }}")
            }
            Expr::If(c, t, f) => {
                let c = self.cond(c);
                let t = self.block_d(t, true);
                match f {
                    Some(f) if matches!(f.as_ref(), Expr::If(..)) => format!("if {c} {t} else {}", self.stmt_expr(f)),
                    Some(f) => format!("if {c} {t} else {}", self.block_d(f, true)),
                    None => format!("if {c} {t}"),
                }
            }
            Expr::Match(subj, arms) => self.match_expr(subj, arms, true),
            // `xs.sort()` as a statement means `xs = xs.sort()`.
            Expr::Method(recv, name, _, line) if PURE_METHODS.contains(&name.as_str()) && matches!(recv.as_ref(), Expr::Ident(_) | Expr::Field(..) | Expr::Index(..)) => {
                self.stmt(&Stmt::Assign { target: (**recv).clone(), op: "=", value: e.clone(), line: *line })
            }
            Expr::Method(recv, name, _, line) if PURE_METHODS.contains(&name.as_str()) => {
                let target = match recv.as_ref() {
                    Expr::Ident(n) => n.clone(),
                    _ => "x".into(),
                };
                self.diag(*line, &format!(".{name}() returns a new value and does not change {target}"), Some(format!("write {target} = {target}.{name}(...)")));
                self.expr(e)
            }
            other => self.expr(other),
        }
    }

    fn bind_pat(&mut self, p: &Pat) {
        match p {
            Pat::Bind(n) => {
                self.locals.insert(n.clone());
            }
            Pat::Tuple(ps) | Pat::Variant(_, ps) => ps.iter().for_each(|p| self.bind_pat(p)),
            _ => {}
        }
    }

    fn pat(&mut self, p: &Pat) -> String {
        match p {
            Pat::Wild => "_".into(),
            Pat::Bind(n) => n.clone(),
            Pat::Int(n) => format!("{n}i64"),
            Pat::Str(s) => rust_str_lit(s),
            Pat::Bool(b) => b.to_string(),
            Pat::Tuple(ps) => format!("({})", ps.iter().map(|p| self.pat(p)).collect::<Vec<_>>().join(", ")),
            Pat::Variant(name, args) if (name == "Ok" || name == "Err") && args.len() == 1 => {
                let p = self.pat(&args[0]);
                if name == "Ok" { format!("Ok({p})") } else { format!("Err(AxErr({p}))") }
            }
            Pat::Variant(name, args) => match self.variants.get(name).cloned() {
                Some((en, fields)) => {
                    if fields.is_empty() {
                        format!("{en}::{name}")
                    } else {
                        if args.len() != fields.len() {
                            self.diag(self.line, &format!("pattern {name} has {} fields, not {}", fields.len(), args.len()), Some(format!("write {name}({})", fields.iter().map(|f| f.name.clone()).collect::<Vec<_>>().join(", "))));
                        }
                        let parts: Vec<String> = fields
                            .iter()
                            .zip(args)
                            .map(|(f, a)| {
                                if let (true, Pat::Bind(x)) = (self.boxed.contains(&(en.clone(), f.name.clone())), a) {
                                    self.unbox.push(x.clone());
                                }
                                format!("{}: {}", f.name, self.pat(a))
                            })
                            .collect();
                        // A JSON tuple (a Python tuple whose length differs by path) matches as an array.
                        if en == "Json" && name == "Arr" {
                            return format!("(Json::Arr {{ {0} }} | Json::Tup {{ {0} }})", parts.join(", "));
                        }
                        // Json::Num also says whether the number was written as a float.
                        if en == "Json" && name == "Num" {
                            return format!("Json::Num {{ {}, .. }}", parts.join(", "));
                        }
                        format!("{en}::{name} {{ {} }}", parts.join(", "))
                    }
                }
                None => {
                    let best = closest(name, self.variants.keys().map(|s| s.as_str())).map(String::from);
                    self.diag(self.line, &format!("unknown variant {name}"), best.as_ref().map(|c| format!("did you mean {c}?")));
                    if let Some(c) = best {
                        self.edit_last(name.clone(), c);
                    }
                    "_".into()
                }
            },
        }
    }

    // A statement, then for a name typed_guard::check_soft lets through, a check rustc makes on its type.
    fn stmt(&mut self, s: &Stmt) -> String {
        let out = self.stmt_inner(s);
        let n = match s {
            Stmt::Let { name: Pat::Bind(n), .. } | Stmt::Assign { target: Expr::Ident(n), op: "=", .. } => n,
            _ => return out,
        };
        match self.rand_order_mark(n) {
            Some(m) => format!("{out}\n    {m}"),
            None => out,
        }
    }

    // `ax_rand_set(&n);` when n is one of the names in RAND_ORDER (a set walked in hash order): it compiles
    // only if n holds strings, whose order CPython draws afresh each run. The comment lets main.rs see that
    // every such name was checked.
    fn rand_order_mark(&self, n: &str) -> Option<String> {
        let names = RAND_ORDER.get()?;
        let kind = names.iter().find_map(|k| k.split_once(':').filter(|x| x.1 == n).map(|x| x.0))?;
        let f = if kind == "h" { "ax_rand_holder" } else { "ax_rand_set" };
        let r = if self.refs.contains(n) {
            format!("&*{n}")
        } else if self.locals.contains(n) {
            format!("&{n}")
        } else {
            format!("&{}", self.global(n)?)
        };
        Some(format!("/*axr:{kind}:{n}*/{f}({r});"))
    }

    fn stmt_inner(&mut self, s: &Stmt) -> String {
        // `(ch,) = d.keys()` / `[x] = xs`: the one item, or Python's ValueError.
        if let Stmt::Assign { target: Expr::List(xs) | Expr::Tuple(xs), op: "=", value, line } = s {
            if let [Expr::Ident(n)] = xs.as_slice() {
                let one = Expr::Call("__one".into(), vec![(None, value.clone())], *line);
                return self.stmt_inner(&Stmt::Assign { target: Expr::Ident(n.clone()), op: "=", value: one, line: *line });
            }
        }
        let line = match s {
            Stmt::Let { line, .. } | Stmt::Assign { line, .. } | Stmt::Expr { line, .. } | Stmt::Return { line, .. } | Stmt::Break { line } | Stmt::Continue { line } | Stmt::Assert { line, .. } => *line,
        };
        self.line = line;
        let set_name = match s {
            Stmt::Let { name: Pat::Bind(n), value, .. } | Stmt::Assign { target: Expr::Ident(n), op: "=", value, .. } => Some((n.clone(), self.is_set_expr(value))),
            _ => None,
        };
        if let Some((n, is_set)) = set_name {
            if is_set {
                self.set_vars.insert(n);
            } else {
                self.set_vars.remove(&n);
            }
        }
        let body = match s {
            Stmt::Let { name: Pat::Bind(n), value, .. } if self.globals.get(n).map_or(false, |g| g.1 == line) => {
                let v = self.expr(value);
                format!("unsafe {{ AXG_{n} = Some({v}); }}")
            }
            Stmt::Let { name, ty, value, mutable, .. } => {
                let v = self.expr(value);
                self.bind_pat(name);
                let p = self.pat(name);
                let t = match (ty, name) {
                    (Some(t), _) => format!(": {}", rust_ty(t)),
                    (None, Pat::Bind(n)) => self.empty_hint(n, line, value),
                    _ => String::new(),
                };
                let (v, t) = match name {
                    Pat::Bind(n) if self.hash_sets.contains(n) => as_hash_set(v, t),
                    Pat::Bind(n) if self.queues.contains(n) => as_wrap("AxQueue", v, t),
                    _ => (v, t),
                };
                // `let` names can be changed too: Python has no constants, and models reassign them.
                let p = if let Pat::Tuple(ps) = name { mut_binds(ps, &p) } else { p };
                format!("let {}{p}{t} = {v};", if *mutable || matches!(name, Pat::Bind(_)) { "mut " } else { "" })
            }
            // Setting a top-level name works even before the script's own line has set it.
            Stmt::Assign { target: Expr::Ident(n), op: "=", value, .. } if self.globals.get(n).map_or(false, |g| g.1 == line) || self.global(n).is_some() => {
                let v = self.expr(value);
                format!("unsafe {{ AXG_{n} = Some({v}); }}")
            }
            // `x = v` for a new name declares it, like `var x = v`.
            Stmt::Assign { target: Expr::Ident(n), op: "=", value, .. }
                if !self.locals.contains(n) && (!self.globals.contains_key(n) || self.own_locals.contains(n)) && !self.variants.contains_key(n) =>
            {
                let v = self.expr(value);
                self.locals.insert(n.clone());
                let mut hint = self.empty_hint(n, line, value);
                // `pd = None` with nothing else ever put in pd: an int slot holding None.
                if hint.is_empty() && matches!(value, Expr::Call(f, a, _) if f == "none" && a.is_empty()) && !self.set_later(n) {
                    hint = ": i64".into();
                }
                let (v, hint) = if self.hash_sets.contains(n) {
                    as_hash_set(v, hint)
                } else if self.queues.contains(n) {
                    as_wrap("AxQueue", v, hint)
                } else {
                    (v, hint)
                };
                format!("let mut {n}{hint} = {v};")
            }
            // `(a, b) = v` unpacks; new names are declared.
            Stmt::Assign { target: Expr::Tuple(xs), op: "=", value, .. } if xs.iter().all(|x| matches!(x, Expr::Ident(_))) => {
                let v = self.expr(value);
                // Python's `h, m = s.split(":")` unpacks a list as well as a tuple.
                let v = if matches!(value, Expr::Tuple(_)) || !(2..=12).contains(&xs.len()) { v } else { format!("AxUnpack::ax_unpack({v})") };
                let names: Vec<String> = xs.iter().map(|x| if let Expr::Ident(n) = x { n.clone() } else { unreachable!() }).collect();
                if names.iter().all(|n| self.locals.contains(n) && !self.globals.contains_key(n)) {
                    let ps: Vec<String> = names.iter().map(|n| if self.refs.contains(n) { format!("*{n}") } else { n.clone() }).collect();
                    format!("({}) = {v};", ps.join(", "))
                } else if names.iter().any(|n| self.locals.contains(n) || self.global(n).is_some()) {
                    // Some names exist already: declare only the new ones, then assign them all.
                    let mut out = String::new();
                    let mut ps = Vec::new();
                    for n in &names {
                        if n == "_" {
                            ps.push("_".into());
                        } else if let Some(g) = self.global(n) {
                            ps.push(g);
                        } else if self.locals.contains(n) {
                            ps.push(if self.refs.contains(n) { format!("*{n}") } else { n.clone() });
                        } else {
                            out.push_str(&format!("let mut {n}; "));
                            self.locals.insert(n.clone());
                            ps.push(n.clone());
                        }
                    }
                    format!("{out}({}) = {v};", ps.join(", "))
                } else {
                    names.iter().for_each(|n| {
                        self.locals.insert(n.clone());
                    });
                    format!("let ({}) = {v};", names.iter().map(|n| if n == "_" { "_".to_string() } else { format!("mut {n}") }).collect::<Vec<_>>().join(", "))
                }
            }
            Stmt::Assign { target, op, value, .. } => {
                let v = self.expr(value);
                let v = match (target, *op) {
                    (Expr::Ident(n), "=") if self.hash_sets.contains(n) => format!("AxSet::from({v})"),
                    (Expr::Ident(n), "=") if self.queues.contains(n) => format!("AxQueue::from({v})"),
                    (Expr::Field(_, f), "=") if self.set_fields.contains(&format!(".{f}")) => format!("AxSet::from({v})"),
                    _ => v,
                };
                match (target, *op) {
                    (Expr::Index(base, idx), "=") => {
                        let b = self.place_mut(base);
                        let i = self.expr(idx);
                        format!("{{ let __v = {v}; {b}.ax_set({i}, __v); }}")
                    }
                    (_, "+=") => {
                        let p = self.place_mut(target);
                        format!("{{ let __v = {v}; let __p = &mut {p}; *__p = Plus::plus(std::mem::take(__p), __v); }}")
                    }
                    (_, op) => {
                        let p = self.place_mut(target);
                        format!("{{ let __v = {v}; {p} {op} __v; }}")
                    }
                }
            }
            Stmt::Expr { expr, .. } => format!("{};", self.stmt_expr(expr)),
            Stmt::Return { value, .. } => {
                let mut v = value.as_ref().map(|v| self.expr(v)).unwrap_or_else(|| "()".into());
                if self.json_ret && matches!(value, Some(Expr::Call(n, a, _)) if n == "none" && a.is_empty()) {
                    // `return None` from a function giving JSON values: JSON null.
                    v = "Json::Null".into();
                } else if self.json_ret && value.is_some() {
                    v = format!("AxToJson::ax_json(&{v})");
                }
                if self.unit_fn && value.is_some() && !self.must {
                    let r = if self.fallible { "return Ok(())" } else { "return" };
                    return format!("    /*@L{line}*/ax_l({line}); {{ let _ = {v}; {r}; }}\n");
                }
                if self.fallible { format!("return Ok({v});") } else { format!("return {v};") }
            }
            Stmt::Break { .. } => format!("break{};", self.loop_label()),
            Stmt::Continue { .. } => format!("continue{};", self.loop_label()),
            Stmt::Assert { cond, .. } => format!("assert!({}, \"assert failed at line {line}\");", self.expr(cond)),
        };
        format!("    /*@L{line}*/ax_l({line}); {body}\n")
    }

    // A place that can be written to.
    fn place_mut(&mut self, e: &Expr) -> String {
        match e {
            Expr::Ident(n) => {
                if let Some(g) = self.global(n) {
                    return g;
                }
                self.check_ident(n);
                if self.refs.contains(n) { format!("(*{n})") } else { n.clone() }
            }
            Expr::Field(b, f) if self.boxed_names.contains(f) => format!("(*{}.{f})", self.place_mut(b)),
            Expr::Field(b, f) if self.shared_fields.contains(f) => format!("(*{}.ax_d()).{f}", self.place(b)),
            Expr::Field(b, f) => format!("{}.{f}", self.place_mut(b)),
            Expr::TupleField(b, i) => format!("{}.{i}", self.place_mut(b)),
            Expr::Index(b, i) => {
                // The key is borrowed unless it reads the collection being changed; then it is computed first.
                let reads_base = root_name(b).map_or(true, |r| format!("{i:?}").contains(&format!("({r:?})")));
                let b = self.place_mut(b);
                let i = if reads_base { format!("&({})", self.expr(i)) } else { self.arg_ref(i) };
                format!("(*{b}.ax_mut({i}))")
            }
            other => {
                self.diag(self.line, "this expression cannot be assigned to", Some("assign to a variable, field or index".into()));
                self.expr(other)
            }
        }
    }

    // A place to read through without cloning (method receivers, field bases).
    fn place(&mut self, e: &Expr) -> String {
        if let Expr::Ident(n) = e {
            if let Some(g) = self.global(n) {
                return g;
            }
        }
        match e {
            Expr::Ident(n) if self.locals.contains(n) => {
                if self.refs.contains(n) { format!("(*{n})") } else { n.clone() }
            }
            Expr::Field(b, f) if self.boxed_names.contains(f) => format!("(*{}.{f})", self.place(b)),
            Expr::Field(b, f) if self.shared_fields.contains(f) => format!("(*{}.ax_d()).{f}", self.place(b)),
            Expr::Field(b, f) if (f == "numerator" || f == "denominator") && !self.structs.values().any(|fs| fs.iter().any(|x| &x.name == f)) => format!("({}).ax_{f}()", self.expr(b)),
            Expr::Field(b, f) => format!("{}.{f}", self.place(b)),
            Expr::TupleField(b, i) => format!("{}.{i}", self.place(b)),
            Expr::Index(b, i) => {
                let b = self.place(b);
                format!("(*{b}.ax_ref({}))", self.arg_ref(i))
            }
            other => format!("({})", self.expr(other)),
        }
    }

    fn operand(&mut self, e: &Expr) -> String {
        match e {
            Expr::Ident(_) | Expr::Field(..) | Expr::TupleField(..) | Expr::Index(..) => self.place(e),
            other => self.expr(other),
        }
    }

    // A reference to a value, for arguments that are only read (keys, searched-for items).
    fn arg_ref(&mut self, e: &Expr) -> String {
        format!("&{}", self.place(e))
    }

    fn check_ident(&mut self, n: &str) {
        if self.locals.contains(n) || self.fns.contains_key(n) || self.globals.contains_key(n) {
            return;
        }
        if self.block_only.contains(n) {
            self.diag(self.line, &format!("{n} was first set inside a block that has ended"), Some(format!("set {n} before that block, e.g. {n} = 0")));
            return;
        }
        let names: Vec<String> = self.locals.iter().cloned().chain(self.fns.keys().cloned()).collect();
        let best = closest(n, names.iter().map(|s| s.as_str())).map(String::from);
        self.diag(self.line, &format!("unknown name '{n}'"), Some(best.as_ref().map_or(format!("set it first: {n} = ..."), |c| format!("did you mean {c}?"))));
        if let Some(c) = best {
            self.edit_last(n.to_string(), c);
        }
    }

    fn call_is_fallible(&self, e: &Expr) -> bool {
        match e {
            Expr::Call(name, _, _) => FALLIBLE_BUILTINS.contains(&name.as_str()) || self.fns.get(name).map_or(false, |f| f.fallible),
            _ => false,
        }
    }

    pub fn expr(&mut self, e: &Expr) -> String {
        match e {
            Expr::Int(n) => format!("{n}i64"),
            Expr::Float(f) => format!("{f:?}f64"),
            Expr::Bool(b) => b.to_string(),
            Expr::Str(parts) => {
                if let [StrPart::Lit(s)] = parts.as_slice() {
                    return format!("String::from({})", rust_str_lit(s));
                }
                let mut fmt = String::new();
                let mut args = Vec::new();
                for p in parts {
                    match p {
                        StrPart::Lit(s) => {
                            // Braces doubled before escaping, so a `\u{1}` escape keeps its own.
                            let lit = rust_str_lit(&s.replace('{', "{{").replace('}', "}}"));
                            fmt.push_str(&lit[1..lit.len() - 1]);
                        }
                        StrPart::Interp(e) => {
                            fmt.push_str("{}");
                            args.push(format!("({}).show()", self.expr(e)));
                        }
                    }
                }
                format!("format!(\"{fmt}\", {})", args.join(", "))
            }
            Expr::Move(n) => {
                if self.global(n).is_none() && self.locals.contains(n) && !self.refs.contains(n) {
                    n.clone()
                } else {
                    self.expr(&Expr::Ident(n.clone()))
                }
            }
            Expr::Ident(n) => {
                if let Some((en, fields)) = self.variants.get(n).cloned() {
                    if fields.is_empty() {
                        return format!("{en}::{n}");
                    }
                }
                if let Some(g) = self.global(n) {
                    return format!("{g}.clone()");
                }
                self.check_ident(n);
                if self.locals.contains(n) {
                    format!("{}.clone()", self.place(e))
                } else if let Some(f) = self.fn_ptr_ty(n) {
                    // A function used as a value (`ops = [add, mul]`) is a fn pointer, so
                    // different functions with the same signature fit in one list or map.
                    format!("({n} as {f})")
                } else if let Some(sig) = self.fns.get(n).filter(|f| f.fallible && n != "main" && !f.params.is_empty()) {
                    // A function that can fail, used as a value (`xs.sort(key=wline)` with wline calling int(s)): its
                    // error stops the run.
                    let ps: Vec<String> = sig.params.iter().enumerate().map(|(i, p)| format!("ax_a{i}: {}", rust_ty(&p.ty))).collect();
                    let args: Vec<String> = (0..sig.params.len()).map(|i| format!("ax_a{i}")).collect();
                    let ret = if matches!(sig.ret, Ty::Unit) { "()".to_string() } else { rust_ty(&sig.ret) };
                    format!("(|{}| -> {ret} {{ match {n}({}) {{ Ok(v) => v, Err(AxErr(e)) => panic!(\"{{}}\", ax_exc_msg(&e)) }} }})", ps.join(", "), args.join(", "))
                } else {
                    n.clone()
                }
            }
            Expr::List(items) => format!("vec![{}]", items.iter().map(|i| self.expr(i)).collect::<Vec<_>>().join(", ")),
            Expr::Map(pairs) => {
                if pairs.is_empty() {
                    "AxM::new()".into()
                } else if self.mixed_map(pairs) {
                    let ps: Vec<String> = pairs
                        .iter()
                        .map(|(k, v)| match v {
                            // An empty list or dict in a JSON record has no item type to name.
                            Expr::List(xs) if xs.is_empty() => format!("(Show::show(&{}), Json::Arr {{ v: vec![] }})", self.expr(k)),
                            Expr::Map(xs) if xs.is_empty() => format!("(Show::show(&{}), Json::Obj {{ v: AxM::ordered() }})", self.expr(k)),
                            _ => format!("(Show::show(&{}), AxToJson::ax_json(&{}))", self.expr(k), self.expr(v)),
                        })
                        .collect();
                    format!("ax_obj(vec![{}])", ps.join(", "))
                } else {
                    // A map of functions holds each in an AxFn, which has the zero value a map needs.
                    let is_fn = |v: &Expr| matches!(v, Expr::Ident(n) if !self.locals.contains(n) && self.global(n).is_none() && self.fns.get(n).is_some_and(|f| !f.fallible && n != "main"));
                    // The pointer type comes from the first named function; lambdas beside it take that type.
                    let fptr = pairs.iter().find_map(|(_, v)| match v {
                        Expr::Ident(n) if is_fn(v) => self.fn_ptr_ty(n),
                        _ => None,
                    });
                    let fns = fptr.is_some() && pairs.iter().all(|(_, v)| is_fn(v) || matches!(v, Expr::Lambda(..)));
                    if fptr.is_none() && pairs.iter().all(|(_, v)| matches!(v, Expr::Lambda(..))) {
                        self.diag(self.line, "a map of lambdas needs the functions' types", Some("make the first one a named function: fn up(s: str) -> str = s.upper(), then {\"up\": up, \"hi\": lambda s: ...}".into()));
                    }
                    let ps: Vec<String> = pairs
                        .iter()
                        .map(|(k, v)| match v {
                            // `{"+": add, "-": lambda a, b: a - b}`: a lambda that uses no outside names is a fn pointer.
                            Expr::Lambda(..) if fns => format!("({}, AxFn(Some(({}) as {})))", self.expr(k), self.lambda_typed(v, fptr.as_deref().unwrap()), fptr.as_deref().unwrap()),
                            _ if fns => format!("({}, AxFn(Some({})))", self.expr(k), self.expr(v)),
                            _ => format!("({}, {})", self.expr(k), self.expr(v)),
                        })
                        .collect();
                    format!("AxM::from([{}])", ps.join(", "))
                }
            }
            Expr::Tuple(items) => format!("({})", items.iter().map(|i| self.expr(i)).collect::<Vec<_>>().join(", ")),
            Expr::Unary(op, a) if *op == "!" => format!("(!{})", self.cond(a)),
            Expr::Unary(op, a) => format!("({op}{})", self.expr(a)),
            // Python-style truth: an empty list, map or string, 0 and "" are false.
            Expr::Binary(op @ ("and" | "or"), a, b) => format!("({} {} {})", self.cond(a), if *op == "and" { "&&" } else { "||" }, self.cond(b)),
            // `x in ("a", "b")`: a tuple of like values is searched as a list.
            Expr::Binary("in", a, b) if matches!(b.as_ref(), Expr::Tuple(_)) => {
                let Expr::Tuple(xs) = b.as_ref() else { unreachable!() };
                self.expr(&Expr::Binary("in", a.clone(), Box::new(Expr::List(xs.clone()))))
            }
            // Python's `"-" * 40`: text repeated.
            Expr::Binary("*", a, b) if [a, b].iter().any(|x| matches!(self.infer_ty(x), Some(Ty::Named(n)) if n == "str")) => {
                let (s, n) = if matches!(self.infer_ty(a), Some(Ty::Named(t)) if t == "str") { (a, b) } else { (b, a) };
                format!("({}).repeat(({}).max(0) as usize)", self.operand(s), self.expr(n))
            }
            // `a - b` on lists used as Python sets: the items of a not in b.
            Expr::Binary("-", a, b) if [a, b].iter().any(|x| match x.as_ref() {
                Expr::List(_) => true,
                _ => self.is_set_expr(x) || matches!(self.infer_ty(x), Some(Ty::List(_))),
            }) => {
                format!("AxSetSub::ax_set_sub({}, {})", self.expr(a), self.expr(b))
            }
            Expr::Binary(op, a, b) => {
                // Comparisons read their operands in place, without copying them.
                let (a, b) = if ["==", "!=", "<", ">", "<=", ">=", "in"].contains(op) { (self.operand(a), self.operand(b)) } else { (self.expr(a), self.expr(b)) };
                match *op {
                    "+" => format!("Plus::plus({a}, {b})"),
                    "and" | "or" => unreachable!(),
                    "~=" => format!("ax_approx({a}, {b})"),
                    "//" => format!("ax_floordiv({a}, {b})"),
                    // Bitwise on ints, union and intersection on lists used as Python sets.
                    "|" => format!("AxBits::ax_bor({a}, {b})"),
                    "&" => format!("AxBits::ax_band({a}, {b})"),
                    "**" => format!("AxPow::ax_pow({a}, {b})"),
                    "in" => format!("AxIn::ax_in(&{b}, &{a})"),
                    "%" if self.py_mode => format!("AxPyMod::ax_pymod({a}, {b})"),
                    op => format!("({a} {op} {b})"),
                }
            }
            // A None put in a JSON slot: JSON null (no type to take ax_none's from).
            Expr::Call(name, args, _) if name == "ax_to_json" && matches!(args.as_slice(), [(_, Expr::Call(n, a, _))] if n == "none" && a.is_empty()) => "Json::Null".to_string(),
            Expr::Call(name, args, line) => {
                self.line = *line;
                let c = self.call(name, args, false);
                if c.starts_with("(return") { c } else { format!("{{ ax_l({line}); {c} }}") }
            }
            // `x.str()` is `str(x)`.
            Expr::Method(recv, name, args, _) if name == "__call" => {
                let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
                format!("({})({})", self.expr(recv), a.join(", "))
            }
            Expr::Method(recv, name, args, line) if name == "str" && args.is_empty() => {
                self.line = *line;
                format!("Show::show(&{})", self.operand(recv))
            }
            // A string read item by item (`enumerate`, `map`, `filter`, `any`, `all`) is its list of characters.
            Expr::Method(recv, name, args, line)
                if ["enumerate", "map", "filter", "any", "all", "sort", "sort_by", "zip", "unique"].contains(&name.as_str()) && matches!(self.infer_ty(recv), Some(Ty::Named(n)) if n == "str") =>
            {
                let chars = Expr::Method(recv.clone(), "chars".into(), vec![], *line);
                self.expr(&Expr::Method(Box::new(chars), name.clone(), args.clone(), *line))
            }
            // s.count("ab") counts a substring.
            Expr::Method(recv, name, args, line) if (name == "count" || name == "count_of") && args.len() == 1 && !matches!(args[0], Expr::Lambda(..)) && matches!(self.infer_ty(recv), Some(Ty::Named(n)) if n == "str") => {
                self.expr(&Expr::Method(recv.clone(), "count_sub".into(), args.clone(), *line))
            }
            // zip(xs, "abc") pairs items with characters.
            Expr::Method(recv, name, args, line) if name == "zip" && args.len() == 1 && matches!(self.infer_ty(&args[0]), Some(Ty::Named(n)) if n == "str") => {
                let chars = Expr::Method(Box::new(args[0].clone()), "chars".into(), vec![], *line);
                self.expr(&Expr::Method(recv.clone(), name.clone(), vec![chars], *line))
            }
            Expr::Method(recv, name, args, line) => {
                self.line = *line;
                if !METHODS.contains(&name.as_str()) {
                    let best = closest(name, METHODS.iter().copied());
                    self.diag(*line, &format!("unknown method .{name}()"), best.map(|c| format!("did you mean .{c}()?")));
                    if let Some(c) = best {
                        self.edit_last(format!(".{name}("), format!(".{c}("));
                    }
                }
                // Changing a list stored in a map or list creates the entry if needed.
                let r = if name == "or" {
                    // The receiver's error is handled here, so no `?` is needed on it.
                    match recv.as_ref() {
                        Expr::Call(n, cargs, l) => {
                            self.line = *l;
                            format!("({})", self.call(n, cargs, true))
                        }
                        other => format!("({})", self.expr(other)),
                    }
                } else if MUTATING_METHODS.contains(&name.as_str()) && through_index(recv) {
                    self.place_mut(recv)
                } else {
                    self.place(recv)
                };
                // A lambda inside the lambda that can fail (`[[int(x) for x in r] for r in rows]`) fails this one too.
                let tries = TRY_METHODS.contains(&name.as_str()) && args.iter().any(|a| matches!(a, Expr::Lambda(_, b) if can_fail(b)));
                // Outside a function that can fail and outside try/except, a failure inside the lambda stops the
                // program with its message, as an uncaught Python exception does (`(int(x) for x in parts)`).
                let tries = tries && (self.fallible || !self.catch.is_empty());
                let unpack = ONE_ARG_METHODS.contains(&name.as_str());
                let ref_arg = ["has", "get_or", "contains", "index_of", "count_of"].contains(&name.as_str());
                let a: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(k, a)| match a {
                        a if k == 0 && ref_arg => self.arg_ref(a),
                        Expr::Lambda(ps, body) => {
                            // Lambda parameters end with the lambda.
                            let outer = self.locals.clone();
                            for p in ps {
                                self.locals.insert(p.clone());
                            }
                            let params = if unpack && ps.len() > 1 { format!("({})", ps.join(", ")) } else { ps.join(", ") };
                            let out = if !tries {
                                let saved = std::mem::replace(&mut self.must, true);
                                let b = self.expr(body);
                                self.must = saved;
                                format!("(|{params}| {b})")
                            } else {
                                let saved = std::mem::replace(&mut self.fallible, true);
                                let saved_must = std::mem::replace(&mut self.must, false);
                                // Inside the lambda a failure is its Err value; the try around it is outside.
                                let saved_catch = std::mem::take(&mut self.catch);
                                let b = self.expr(body);
                                self.catch = saved_catch;
                                self.fallible = saved;
                                self.must = saved_must;
                                format!("(|{params}| -> R<_> {{ Ok({b}) }})")
                            };
                            self.locals = outer;
                            out
                        }
                        other => self.expr(other),
                    })
                    .collect();
                // `t[1:3]` with number bounds may be on a tuple, which only a const-bound slice can give.
                let lit = |x: Option<&Expr>| match x {
                    Some(Expr::Int(k)) if *k >= 0 && *k < 64 => Some(k.to_string()),
                    _ => None,
                };
                if name == "slice" && !tries && (1..=2).contains(&args.len()) {
                    let b = if args.len() == 1 { Some("{usize::MAX}".to_string()) } else { lit(args.get(1)) };
                    if let (Some(x), Some(y)) = (lit(args.first()), b) {
                        return format!("{{ ax_l({line}); AxTSlice::<{x}, {y}>::ax_tslice(&{r}) }}");
                    }
                }
                // `s.slice(a)` runs to the end.
                let mut a = a;
                if name == "slice" && a.len() == 1 {
                    a.push("i64::MAX".into());
                }
                // `d.get(k, {})` / `d.get(k, [])`: an empty value of whatever d holds (a JSON object's own kind,
                // or the map's value type), where a bare `{}` has no type of its own.
                let empty = |x: Option<&Expr>| match x {
                    Some(Expr::Map(ps)) if ps.is_empty() => Some("get_or_emap"),
                    Some(Expr::List(xs)) if xs.is_empty() => Some("get_or_elist"),
                    _ => None,
                };
                let (name, a) = match empty(args.get(1)) {
                    Some(n) if name == "get_or" && a.len() == 2 => (n.to_string(), vec![a[0].clone()]),
                    _ => (name.clone(), a),
                };
                // `x.round(2)` rounds to decimal places.
                let name = if name == "round" && a.len() == 1 { "round_to" } else { name.as_str() };
                // `xs.sort_by(f, true)` sorts largest first, keeping equal items in order (Python's reverse=True).
                let name = if name == "sort_by" && a.len() == 2 { "sort_by_rev" } else { name };
                // `s.re_sub(p, g => ...)` builds each replacement from the match groups.
                let name = if name == "re_sub" && matches!(args.get(1), Some(Expr::Lambda(..))) { "re_sub_with" } else { name };
                if tries && !self.catch.is_empty() {
                    let lab = self.catch.last().unwrap().clone();
                    let brk = if lab.starts_with("ax_tryv") { " None" } else { "" };
                    format!("{{ ax_l({line}); match {r}.ax_try_{name}({}) {{ Ok(v) => v, Err(e) => {{ {lab} = Some(AxErr::from(e)); break '{lab}{brk}; }} }} }}", a.join(", "))
                } else if tries {
                    format!("{{ ax_l({line}); {r}.ax_try_{name}({})? }}", a.join(", "))
                } else {
                    format!("{{ ax_l({line}); {r}.ax_{name}({}) }}", a.join(", "))
                }
            }
            // `d.year`, `d.weekday` without brackets read a date's parts.
            Expr::Field(b, f) if ["year", "month", "day", "weekday", "numerator", "denominator"].contains(&f.as_str()) && !self.structs.values().any(|fs| fs.iter().any(|x| &x.name == f)) => {
                format!("({}).ax_{f}()", self.expr(b))
            }
            Expr::Field(..) | Expr::TupleField(..) => format!("{}.clone()", self.place(e)),
            Expr::Index(..) => format!("{}.clone()", self.place(e)),
            Expr::Lambda(params, body) => {
                let outer = self.locals.clone();
                for p in params {
                    self.locals.insert(p.clone());
                }
                let saved = std::mem::replace(&mut self.must, true);
                let out = format!("(|{}| {})", params.join(", "), self.expr(body));
                self.must = saved;
                self.locals = outer;
                out
            }
            Expr::Try(inner) if self.must => match inner.as_ref() {
                Expr::Call(name, args, line) => {
                    self.line = *line;
                    format!("ax_must({})", self.call(name, args, true))
                }
                other => format!("ax_must(ax_q({}))", self.expr(other)),
            },
            Expr::Try(inner) if !self.catch.is_empty() => {
                let lab = self.catch.last().unwrap().clone();
                let v = match inner.as_ref() {
                    Expr::Call(name, args, line) => {
                        self.line = *line;
                        self.call(name, args, true)
                    }
                    other => format!("ax_q({})", self.expr(other)),
                };
                let brk = if lab.starts_with("ax_tryv") { " None" } else { "" };
                format!("(match {v} {{ Ok(v) => v, Err(e) => {{ {lab} = Some(AxErr::from(e)); break '{lab}{brk}; }} }})")
            }
            Expr::Try(inner) => {
                if !self.fallible {
                    self.diag(self.line, "'?' used in a function that cannot fail", Some("add ! after the function's return type".into()));
                }
                match inner.as_ref() {
                    Expr::Call(name, args, line) => {
                        self.line = *line;
                        format!("{}?", self.call(name, args, true))
                    }
                    // ax_q lets `?` follow conversions that cannot fail on numbers (`2.5.int()`).
                    other => format!("ax_q({})?", self.expr(other)),
                }
            }
            // A try whose value is used (`try: return f(s)` at a function's end, where the return became its value):
            // a failure breaks out with None and the handler gives the value.
            Expr::If(c, t, Some(h)) if is_try(c) => {
                self.try_n += 1;
                let lab = format!("ax_tryv{}", self.try_n);
                self.catch.push(lab.clone());
                let body = self.block(t);
                self.catch.pop();
                let handler = self.block(h);
                format!("{{ let mut {lab}: Option<AxErr> = None; match '{lab}: {{ Some({body}) }} {{ Some(v) => v, None => {{ let AxErr(ax_err) = {lab}.unwrap(); let _ = &ax_err; {handler} }} }} }}")
            }
            Expr::If(c, t, f) => {
                let c = self.cond(c);
                let t = self.block(t);
                match f {
                    Some(f) => {
                        let f = if matches!(f.as_ref(), Expr::If(..)) { self.expr(f) } else { self.block(f) };
                        format!("if {c} {t} else {f}")
                    }
                    None => format!("if {c} {t}"),
                }
            }
            Expr::Match(subj, arms) => self.match_expr(subj, arms, false),
            Expr::For(p, iter, body) => {
                // Assigning to the loop variable edits the list in place.
                if let (Pat::Bind(n), Expr::Ident(_) | Expr::Index(..) | Expr::Field(..) | Expr::TupleField(..)) = (p, iter.as_ref()) {
                    // (Not when the body also reads the list's owner: it then works on a copy of each row.)
                    let root_read = root_name(iter).map_or(false, |r| { let t = format!("{body:?}"); t.contains(&format!("Ident({r:?})")) || t.contains(&format!("Move({r:?})")) });
                    if changes(body, n) && root_name(iter).is_some() && !root_read {
                        let it = self.place_mut(iter);
                        self.locals.insert(n.clone());
                        self.refs.insert(n.clone());
                        self.open_loop();
                        let b = self.block_d(body, true);
                        let lab = self.close_loop();
                        self.refs.remove(n);
                        return format!("{lab}for {n} in {it}.iter_mut() {b}");
                    }
                    // Python changes the list's own items (`r[1] = v` on each row); a copy of each row would lose that.
                    if crate::infer::py_on() && root_read && changes_in_place(body, n) && !matches!(self.infer_ty(iter), Some(Ty::List(x)) if matches!(x.as_ref(), Ty::Named(k) if k != "Json")) {
                        self.diag(self.line, &format!("the loop changes each item of a list in place ({n}[...] = ...) while also reading the list's owner"), None);
                    }
                }
                let it = match iter.as_ref() {
                    // A range is counted through directly instead of being built as a list.
                    Expr::Range(a, b) => format!("({}..{})", self.expr(a), self.expr(b)),
                    // range(a, b, k) and range(b, a, -k): stepped through directly too.
                    Expr::Method(r, m, k, _) if m == "step" && k.len() == 1 && matches!(r.as_ref(), Expr::Range(..)) => {
                        let Expr::Range(a, b) = r.as_ref() else { unreachable!() };
                        format!("({}..{}).step_by(ax_step_by({}))", self.expr(a), self.expr(b), self.expr(&k[0]))
                    }
                    Expr::Method(r, m, k, _) if m == "step" && k.len() == 1 && matches!(r.as_ref(), Expr::Method(q, rv, ra, _) if rv == "rev" && ra.is_empty() && matches!(q.as_ref(), Expr::Range(..))) => {
                        let Expr::Method(q, _, _, _) = r.as_ref() else { unreachable!() };
                        let Expr::Range(a, b) = q.as_ref() else { unreachable!() };
                        format!("({}..{}).rev().step_by(ax_step_by({}))", self.expr(a), self.expr(b), self.expr(&k[0]))
                    }
                    // A list that is still needed is read item by item instead of copied whole.
                    e @ (Expr::Ident(_) | Expr::Field(..) | Expr::TupleField(..) | Expr::Index(..))
                        if self.borrow_loops && root_name(e).map_or(false, |r| self.locals.contains(r) && self.global(r).is_none()) =>
                    {
                        format!("{}.ax_iter()", self.place(e))
                    }
                    // Python's `for row in csv.reader(src)`: rows one at a time.
                    Expr::Method(src, m, a, _) if m == "csv_rows" && a.is_empty() => {
                        let s = self.expr(src);
                        if matches!(src.as_ref(), Expr::Ident(_) | Expr::Field(..) | Expr::Index(..) | Expr::TupleField(..)) { format!("AxCsvStream::ax_csv_iter(({s}).clone())") } else { format!("AxCsvStream::ax_csv_iter({s})") }
                    }
                    other => format!("AxEach::ax_each({})", self.expr(other)),
                };
                let outer = self.locals.clone();
                let pat_src = p;
                self.bind_pat(p);
                // A loop variable over a computed list is a fresh copy, so it may be set.
                let p = match p {
                    Pat::Bind(n) if changes(body, n) => format!("mut {n}"),
                    Pat::Tuple(ps) if ps.iter().any(|q| matches!(q, Pat::Bind(n) if changes(body, n))) => {
                        let names: Vec<String> = ps.iter().map(|q| match q { Pat::Bind(n) if changes(body, n) => format!("mut {n}"), q => self.pat(q) }).collect();
                        format!("({})", names.join(", "))
                    }
                    p => self.pat(p),
                };
                // Python's `for a, b, w in rows` also unpacks lists of that many items.
                let nested = matches!(pat_src, Pat::Tuple(ps) if ps.iter().any(|q| matches!(q, Pat::Tuple(_))));
                let it = match iter.as_ref() {
                    Expr::Range(..) => it,
                    // `for sku, (q, n) in pairs` where each pair holds a list: unpack each level.
                    _ if nested => format!("{it}.map(|ax_x| {})", unpack_levels(pat_src, "ax_x", &mut 0)),
                    _ if p.starts_with('(') && (2..=12).contains(&p.matches(',').count().saturating_add(1)) && !p[1..].contains('(') => {
                        format!("{it}.map(|ax_x| AxUnpack::ax_unpack(ax_x))")
                    }
                    _ => it,
                };
                self.open_loop();
                let b = self.block_d(body, true);
                let lab = self.close_loop();
                self.locals = outer;
                // Passed to a Python library call that may change it (`w.writerow(r)`): a fresh copy too.
                let p = if !p.starts_with("mut ") && !p.starts_with('(') && b.contains(&format!("ax_pyb_argm(&mut {p})")) { format!("mut {p}") } else { p };
                format!("{lab}for {p} in {it} {b}")
            }
            // `while true:` is Rust's `loop`, so a function may end in it and return from inside.
            Expr::While(c, body) if matches!(c.as_ref(), Expr::Bool(true)) => {
                self.open_loop();
                let b = self.block_d(body, true);
                format!("{}loop {b}", self.close_loop())
            }
            Expr::While(c, body) => {
                let c = self.cond(c);
                self.open_loop();
                let b = self.block_d(body, true);
                format!("{}while {c} {b}", self.close_loop())
            }
            Expr::Range(a, b) => format!("({}..{}).collect::<Vec<i64>>()", self.expr(a), self.expr(b)),
            Expr::Block(..) => self.block(e),
        }
    }

    fn match_expr(&mut self, subj: &Expr, arms: &[(Pat, Expr)], discard: bool) -> String {
        let has_str = arms.iter().any(|(p, _)| matches!(p, Pat::Str(_)));
        let handles_err = arms.iter().any(|(p, _)| matches!(p, Pat::Variant(n, _) if n == "Ok" || n == "Err"));
        let s = match subj {
            // Matching Ok/Err handles the call's error, so it needs no `?`.
            Expr::Call(n, args, line) if handles_err => {
                self.line = *line;
                format!("{{ ax_l({line}); {} }}", self.call(n, args, true))
            }
            other => self.expr(other),
        };
        let s = if has_str { format!("({s}).as_str()") } else { s };
        let mut out = format!("match {s} {{\n");
        for (p, body) in arms {
            let outer = self.locals.clone();
            self.bind_pat(p);
            self.unbox.clear();
            let p = self.pat(p);
            let unbox: String = std::mem::take(&mut self.unbox).iter().map(|x| format!("let mut {x} = *{x}.0; ")).collect();
            // `_ => {}` in a match that gives a value is that value's empty or zero form.
            let b = match body {
                Expr::Block(st, None) if st.is_empty() && !discard => "{ Default::default() }".to_string(),
                body => self.block_d(body, discard),
            };
            self.locals = outer;
            let b = if unbox.is_empty() { b } else { format!("{{ {unbox}{b} }}") };
            out.push_str(&format!("        {p} => {b},\n"));
        }
        out.push_str("    }");
        out
    }

    fn call(&mut self, name: &str, args: &[(Option<String>, Expr)], tried: bool) -> String {
        if !tried && self.call_is_fallible(&Expr::Call(name.into(), vec![], 0)) {
            self.diag(self.line, &format!("{name}() can fail but its error is not handled"), Some(format!("write {name}(...)? to pass the error up")));
        }
        // A copy that is only wrong when the value is not shared: the Rust compiler checks the value's type,
        // and an error there reports the message carried (hex) in the comment.
        if name == "__shared" && (args.len() == 2 || args.len() == 3) {
            let hex = match &args[1].1 {
                Expr::Str(parts) => parts.iter().map(|p| if let StrPart::Lit(t) = p { t.clone() } else { String::new() }).collect::<String>(),
                _ => String::new(),
            };
            let f = match args.get(2).map(|a| &a.1) {
                Some(Expr::Int(2)) => "ax_shared_aug",
                Some(_) => "ax_shared_whole",
                None => "ax_shared",
            };
            return format!("{f}(&{}) /*axsh:{hex}*/", self.place(&args[0].1));
        }
        // `str({"id": i, "error": m})` writes the keys in the order given, as Python does.
        if name == "str" && args.len() == 1 {
            if let Expr::Map(pairs) = &args[0].1 {
                if !pairs.is_empty() && pairs.iter().all(|(k, _)| matches!(k, Expr::Str(_))) {
                    let ps: Vec<String> = pairs.iter().map(|(k, v)| format!("(Show::show(&{}), AxToJson::ax_json(&{}))", self.expr(k), self.expr(v))).collect();
                    return format!("ax_obj(vec![{}]).show()", ps.join(", "));
                }
            }
        }
        // Python's print(a, b, sep=", ", end=""): written as text, with the given separator and ending.
        if name == "print" && args.iter().any(|(n, _)| matches!(n.as_deref(), Some("end" | "sep"))) {
            let named = |k: &str| args.iter().find(|(n, _)| n.as_deref() == Some(k)).map(|(_, a)| a.clone());
            let sep = named("sep").map(|e| format!("Show::show(&{})", self.expr(&e))).unwrap_or_else(|| "String::from(\" \")".to_string());
            let end = named("end").map(|e| format!("Show::show(&{})", self.expr(&e))).unwrap_or_else(|| "\"\\n\"".to_string());
            let items: Vec<String> = args.iter().filter(|(n, _)| n.is_none()).map(|(_, a)| format!("Show::show(&{})", self.print_arg(a))).collect();
            return format!("{{ use std::io::Write; let __s = [{}].join(&{sep}); print!(\"{{}}{{}}\", __s, {end}); let _ = std::io::stdout().flush(); }}", items.join(", "));
        }
        let plain: Vec<String> = args.iter().map(|(_, a)| if name == "print" { self.print_arg(a) } else { self.expr(a) }).collect();
        match name {
            // print(a, b) writes them with a space between, like Python.
            "print" if plain.len() > 1 => return format!("print([{}].join(\" \"))", plain.iter().map(|p| format!("Show::show(&{p})")).collect::<Vec<_>>().join(", ")),
            // A local of the same name (`date = parts[1]`) would hide the runtime's function.
            "print" | "read" | "write" | "args" | "input" | "stdin" | "lines" | "json" | "csv" | "date" | "abs" | "min" | "max" | "heap" if self.locals.contains(name) => return format!("crate::{name}({})", plain.join(", ")),
            "print" | "read" | "write" | "args" | "input" | "stdin" | "lines" | "json" | "csv" | "date" | "abs" | "min" | "max" | "heap" => return format!("{name}({})", plain.join(", ")),
            "__readline" => return "ax_readline()".to_string(),
            "__one" => return format!("ax_one({})", plain.join(", ")),
            "__eprint" => return format!("{{ use std::io::Write; let _ = std::io::stdout().flush(); eprint!(\"{{}}\", {}); }}", plain.join(", ")),
            "__try_read" => return format!("std::fs::read_to_string({}.to_string()).unwrap_or_default()", plain.join(", ")),
            "str" => return format!("({}).show()", plain.join(", ")),
            "none" => return "ax_none()".to_string(),
            "__global" => return "()".to_string(),
            "__err" => return "ax_exc_msg(&ax_err)".to_string(),
            "__err_raw" => return "ax_err.clone()".to_string(),
            "__exc" => return format!("ax_exc({})", plain.join(", ")),
            "__exc_is" => return format!("ax_exc_is(&ax_err, {})", plain.join(", ")),
            "__exc_obj" => return format!("ax_exc_obj({})", plain.join(", ")),
            "__exc_get" => return "ax_exc_get(&ax_err)".to_string(),
            "__default" => return "AxFresh::ax_fresh()".to_string(),
            // The value type's zero (0, "", [], {}), for a key put in by `m.setdefault(k, [])`.
            "__zero" => return "Default::default()".to_string(),
            "__truediv" => return format!("AxTrueDiv::ax_truediv({})", plain.join(", ")),
            "fail" if !self.catch.is_empty() => {
                let lab = self.catch.last().unwrap().clone();
                let brk = if lab.starts_with("ax_tryv") { " None" } else { "" };
                return format!("{{ {lab} = Some(AxErr({})); break '{lab}{brk}; }}", plain.join(", "));
            }
            "fail" => {
                if !self.fallible {
                    self.diag(self.line, "fail() used in a function that cannot fail", Some("add ! after the function's return type".into()));
                }
                return format!("(return Err(AxErr({})))", plain.join(", "));
            }
            _ => {}
        }
        if let Some(fields) = self.structs.get(name).cloned() {
            return self.construct(name, name, name, &fields, args, plain);
        }
        if let Some((en, fields)) = self.variants.get(name).cloned() {
            let made = self.construct(&format!("{en}::{name}"), name, &en, &fields, args, plain);
            if en == "Json" && name == "Num" {
                return made.replacen("Json::Num {", "Json::Num { f: true,", 1);
            }
            return made;
        }
        if let Some(sig) = self.fns.get(name) {
            let params = sig.params.clone();
            if params.len() != args.len() {
                let sigtext = params.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(", ");
                self.diag(self.line, &format!("{name}() takes {} arguments, got {}", params.len(), args.len()), Some(format!("call {name}({sigtext})")));
            }
            // Named arguments may come in any order.
            let mut ordered = Vec::new();
            for (i, p) in params.iter().enumerate() {
                let found = args.iter().position(|(n, _)| n.as_deref() == Some(p.name.as_str())).or(if args.get(i).map_or(false, |(n, _)| n.is_none()) { Some(i) } else { None });
                // A variable changed by the function but converted on the way in (`f(ax_to_json(xs))`): the
                // function would change the converted copy and the caller would never see it.
                if let Some(j) = found {
                    if p.is_mut && matches!(&args[j].1, Expr::Call(f, a, _) if (f == "ax_to_json" || f == "ax_tup_json") && matches!(a.first().map(|x| &x.1), Some(Expr::Ident(_) | Expr::Move(_) | Expr::Field(..) | Expr::Index(..)))) {
                        self.diag(self.line, &format!("{name}() changes `{}` but it holds a different type here", p.name), Some("give the variable one type".into()));
                    }
                }
                match found {
                    // A value that is not a variable (`f(0)`) is passed as a fresh copy the function may change.
                    Some(j) if p.is_mut && !matches!(args[j].1, Expr::Ident(_) | Expr::Move(_) | Expr::Field(..) | Expr::Index(..) | Expr::TupleField(..)) => ordered.push(format!("&mut ({})", plain[j])),
                    // A Python object is a shared handle: the function gets its own copy of the handle (changes reach
                    // the same object), so the caller's place is not borrowed while the other arguments are read.
                    // (Only `self`: other names passed by reference may be set to a new object, which the caller must see.)
                    Some(j) if p.is_mut && p.name == "self_" && matches!(&p.ty, Ty::Named(n) if self.marked.contains(n)) => ordered.push(format!("&mut ({})", plain[j])),
                    Some(j) if p.is_mut => ordered.push(format!("&mut {}", self.place_mut(&args[j].1))),
                    Some(j) if self.set_params.get(name).is_some_and(|v| v.contains(&i)) && !matches!(&args[j].1, Expr::Ident(n) | Expr::Move(n) if self.hash_sets.contains(n)) => {
                        ordered.push(format!("AxSet::from({})", plain[j]))
                    }
                    Some(j) => ordered.push(plain[j].clone()),
                    None => {}
                }
            }
            return format!("{name}({})", ordered.join(", "));
        }
        // pybridge: calls into the Python runtime (pyrt.rs).
        if name.starts_with("ax_pyb") && crate::pybridge::active() {
            // A name given to Python by place (its changes are written back); a lambda moves what it uses.
            if name == "ax_pyb_argm" && args.len() == 1 {
                let a = match &args[0].1 {
                    Expr::Move(n) => Expr::Ident(n.clone()),
                    other => other.clone(),
                };
                return format!("ax_pyb_argm(&mut {})", self.place_mut(&a));
            }
            if name.starts_with("ax_pyb_fn") {
                return format!("{name}({})", plain.join(", ").replacen("(|", "(move |", 1));
            }
            return format!("{name}({})", plain.join(", "));
        }
        if name == "ax_json_null" {
            return "Json::Null".into();
        }
        if self.locals.contains(name) || name == "ax_to_json" || matches!(name, "ax_tup_json" | "ax_jint" | "ax_jas_int" | "ax_jas_str" | "ax_jkey_str" | "ax_unsupported" | "ax_row" | "ax_json_dec" | "ax_json_strs" | "ax_json_ints" | "ax_subset") {
            return format!("{name}({})", plain.join(", "));
        }
        let known: Vec<String> = self.fns.keys().cloned().chain(BUILTINS.iter().map(|s| s.to_string())).chain(self.structs.keys().cloned()).chain(self.variants.keys().cloned()).collect();
        let best = closest(name, known.iter().map(|s| s.as_str())).map(String::from);
        self.diag(self.line, &format!("unknown function {name}()"), best.as_ref().map(|c| format!("did you mean {c}()?")));
        if let Some(c) = best {
            self.edit_last(format!("{name}("), format!("{c}("));
        }
        format!("{name}({})", plain.join(", "))
    }

    // Each field takes, in order of preference: an argument named after it (`total: t`), a plain
    // variable of the same name (`total`), the next plain argument, or the field's default.
    fn construct(&mut self, path: &str, name: &str, owner: &str, fields: &[Field], args: &[(Option<String>, Expr)], plain: Vec<String>) -> String {
        if fields.is_empty() && args.is_empty() {
            return path.to_string();
        }
        let mut used = vec![false; args.len()];
        let mut slot: Vec<Option<usize>> = vec![None; fields.len()];
        for (fi, f) in fields.iter().enumerate() {
            let by_name = args.iter().enumerate().position(|(j, (n, a))| {
                !used[j] && (n.as_deref() == Some(f.name.as_str()) || (n.is_none() && matches!(a, Expr::Ident(v) | Expr::Move(v) if *v == f.name)))
            });
            if let Some(j) = by_name {
                used[j] = true;
                slot[fi] = Some(j);
            }
        }
        for fi in 0..fields.len() {
            if slot[fi].is_some() {
                continue;
            }
            if let Some(j) = (0..args.len()).find(|&j| !used[j] && args[j].0.is_none()) {
                used[j] = true;
                slot[fi] = Some(j);
            }
        }
        let missing: Vec<&str> = fields.iter().zip(&slot).filter(|(f, s)| s.is_none() && f.default.is_none()).map(|(f, _)| f.name.as_str()).collect();
        let extra: Vec<String> = args.iter().zip(&used).filter(|(_, u)| !**u).map(|((n, _), _)| n.clone().unwrap_or_else(|| "a value".into())).collect();
        if !missing.is_empty() || !extra.is_empty() {
            let want = fields.iter().filter(|f| f.default.is_none()).map(|f| format!("{}: ...", f.name)).collect::<Vec<_>>().join(", ");
            let msg = if !missing.is_empty() { format!("{name} is missing {}", missing.join(", ")) } else { format!("{name} has no field for {}", extra.join(", ")) };
            self.diag(self.line, &msg, Some(format!("write {name}({want})")));
        }
        let mut parts = Vec::new();
        for (f, s) in fields.iter().zip(&slot) {
            let boxed = self.boxed.contains(&(owner.to_string(), f.name.clone()));
            match (s, &f.default) {
                (Some(j), _) if boxed => parts.push(format!("{}: AxB::new({})", f.name, plain[*j])),
                (Some(j), _) if self.set_fields.contains(&format!(".{}", f.name)) => parts.push(format!("{}: AxSet::from({})", f.name, plain[*j])),
                (Some(j), _) => parts.push(format!("{}: {}", f.name, plain[*j])),
                (None, Some(d)) => {
                    let v = self.expr(d);
                    parts.push(format!("{}: {v}", f.name));
                }
                _ => {}
            }
        }
        if self.marked.contains(owner) && !path.contains("::") {
            return format!("{path}::ax_new({path}_D {{ {} }})", parts.join(", "));
        }
        format!("{path} {{ {} }}", parts.join(", "))
    }
}

fn slug(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect()
}

// The condition of the if that a Python try statement becomes.
fn is_try(c: &Expr) -> bool {
    matches!(c, Expr::Call(n, a, _) if n == "__try" && a.is_empty())
}

// A `break` of this loop (not one of a loop inside it).
fn own_break(body: &Expr) -> bool {
    fn go(e: &Expr) -> bool {
        match e {
            Expr::Block(ss, t) => ss.iter().any(|s| match s {
                Stmt::Break { .. } => true,
                Stmt::Expr { expr, .. } => go(expr),
                _ => false,
            }) || t.as_ref().is_some_and(|t| go(t)),
            Expr::If(_, a, b) => go(a) || b.as_ref().is_some_and(|b| go(b)),
            Expr::Match(_, arms) => arms.iter().any(|a| go(&a.1)),
            _ => false,
        }
    }
    go(body)
}

// Python's `in` on a list scans it; a local list (or a field of the program's classes) that is searched with
// `in` and otherwise only added to, popped, counted, sorted or walked (a `seen` set, a `visited` list) is
// kept as an AxSet, a Vec with a hash index, so `in` takes O(1) as on a CPython set. A local list taken from
// the front (`q.popleft()`, `q.pop(0)`) and otherwise only added to, popped, read or walked is kept as an
// AxQueue (a VecDeque), so taking from the front is O(1) as on a CPython deque. Builds with both off are
// tried when one fails.
// Names whose sets the program walks in hash order, tagged `s:` or `h:` (see typed_guard::check_soft).
pub static RAND_ORDER: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
pub static FAST_SETS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

#[derive(Clone, Copy, PartialEq)]
enum FsMode {
    Set,
    Queue,
}

fn as_wrap(kind: &str, v: String, hint: String) -> (String, String) {
    let hint = match hint.strip_prefix(": Vec<").and_then(|h| h.strip_suffix('>')) {
        Some(inner) => format!(": {kind}<{inner}>"),
        None => hint,
    };
    (format!("{kind}::from({v})"), hint)
}

fn as_hash_set(v: String, hint: String) -> (String, String) {
    as_wrap("AxSet", v, hint)
}

fn fast_sets(body: &Expr, params: &[Param]) -> HashSet<String> {
    fast_places(body, params, FsMode::Set).into_iter().filter(|k| !k.starts_with('.')).collect()
}

fn fast_queues(body: &Expr, params: &[Param]) -> HashSet<String> {
    fast_places(body, params, FsMode::Queue).into_iter().filter(|k| !k.starts_with('.')).collect()
}

fn fast_places(body: &Expr, params: &[Param], mode: FsMode) -> HashSet<String> {
    let mut good = HashSet::new();
    let mut bad: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
    fs_expr(body, mode, &mut good, &mut bad);
    good.retain(|n| !bad.contains(n));
    good
}

// Fields (as ".name") used only as sets in every function of the program.
fn fast_set_fields(items: &[Item]) -> HashSet<String> {
    let mut good = HashSet::new();
    let mut bad = HashSet::new();
    for it in items {
        match it {
            Item::Fn { body, .. } | Item::Test { body, .. } => fs_expr(body, FsMode::Set, &mut good, &mut bad),
            Item::Type { fields, .. } => {
                // A field with a default value (`seen: [int] = []`) is filled in where AxSet is not known.
                for f in fields.iter().filter(|f| f.default.is_some()) {
                    bad.insert(format!(".{}", f.name));
                }
            }
            _ => {}
        }
    }
    good.into_iter().filter(|k| k.starts_with('.') && !bad.contains(k)).collect()
}

// Methods an AxSet answers (its own, or the Vec's through Deref, which all read through &self).
const FS_METHODS: [&str; 24] = [
    "contains", "push", "pop", "remove", "len", "is_empty", "index_of", "sort", "min", "max", "sum", "any", "all", "count", "map", "filter",
    "first", "last", "join", "rev", "sort_by", "enumerate", "count_of", "fold",
];
// Methods an AxQueue has.
const FQ_METHODS: [&str; 7] = ["push", "pop", "pop_at", "insert", "len", "is_empty", "contains"];

// The name (or ".field") of a place that could be kept as an AxSet or AxQueue.
fn fs_key(e: &Expr) -> Option<String> {
    match e {
        Expr::Ident(n) | Expr::Move(n) => Some(n.clone()),
        Expr::Field(_, f) => Some(format!(".{f}")),
        _ => None,
    }
}

fn fs_base(e: &Expr, mode: FsMode, good: &mut HashSet<String>, bad: &mut HashSet<String>) {
    if let Expr::Field(b, _) = e {
        fs_expr(b, mode, good, bad);
    }
}

fn fs_pat(p: &Pat, bad: &mut HashSet<String>) {
    match p {
        Pat::Bind(n) => {
            bad.insert(n.clone());
        }
        Pat::Tuple(ps) | Pat::Variant(_, ps) => ps.iter().for_each(|q| fs_pat(q, bad)),
        _ => {}
    }
}

fn fs_root(e: &Expr) -> Option<String> {
    match e {
        Expr::Index(b, _) | Expr::TupleField(b, _) => fs_root(b).or_else(|| fs_key(b)),
        Expr::Field(b, _) => fs_key(e).or_else(|| fs_root(b)),
        _ => None,
    }
}

fn fs_expr(e: &Expr, mode: FsMode, good: &mut HashSet<String>, bad: &mut HashSet<String>) {
    let go = |x: &Expr, good: &mut HashSet<String>, bad: &mut HashSet<String>| fs_expr(x, mode, good, bad);
    let methods: &[&str] = if mode == FsMode::Set { &FS_METHODS } else { &FQ_METHODS };
    match e {
        Expr::Ident(n) | Expr::Move(n) => {
            bad.insert(n.clone());
        }
        Expr::Method(r, m, args, _) if fs_key(r).is_some() && methods.contains(&m.as_str()) => {
            let k = fs_key(r).unwrap();
            let front = m == "pop_at" && matches!(args.as_slice(), [Expr::Int(0)]);
            if mode == FsMode::Set && m == "contains" || mode == FsMode::Queue && front {
                good.insert(k.clone());
            }
            if m == "pop_at" && !front {
                bad.insert(k);
            }
            fs_base(r, mode, good, bad);
            args.iter().for_each(|a| go(a, good, bad));
        }
        Expr::Binary("in", a, b) if fs_key(b).is_some() => {
            if mode == FsMode::Set {
                good.insert(fs_key(b).unwrap());
            }
            fs_base(b, mode, good, bad);
            go(a, good, bad);
        }
        Expr::Index(b, i) if mode == FsMode::Queue && matches!(b.as_ref(), Expr::Ident(_) | Expr::Move(_)) => go(i, good, bad),
        Expr::For(p, it, b) => {
            fs_pat(p, bad);
            if fs_key(it).is_some() {
                fs_base(it, mode, good, bad);
            } else {
                go(it, good, bad);
            }
            go(b, good, bad);
        }
        Expr::Unary("!", x) if fs_key(x).is_some() => fs_base(x, mode, good, bad),
        Expr::If(c, t, f) => {
            if fs_key(c).is_some() {
                fs_base(c, mode, good, bad);
            } else {
                go(c, good, bad);
            }
            go(t, good, bad);
            if let Some(f) = f {
                go(f, good, bad);
            }
        }
        Expr::While(c, b) => {
            if fs_key(c).is_some() {
                fs_base(c, mode, good, bad);
            } else {
                go(c, good, bad);
            }
            go(b, good, bad);
        }
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) => {}
        Expr::Str(ps) => ps.iter().for_each(|p| {
            if let StrPart::Interp(x) = p {
                go(x, good, bad)
            }
        }),
        Expr::List(xs) | Expr::Tuple(xs) => xs.iter().for_each(|x| go(x, good, bad)),
        Expr::Map(ps) => ps.iter().for_each(|(k, v)| {
            go(k, good, bad);
            go(v, good, bad);
        }),
        Expr::Field(x, f) => {
            bad.insert(format!(".{f}"));
            go(x, good, bad);
        }
        Expr::Unary(_, x) | Expr::TupleField(x, _) | Expr::Try(x) => go(x, good, bad),
        Expr::Binary(_, a, b) | Expr::Index(a, b) | Expr::Range(a, b) => {
            go(a, good, bad);
            go(b, good, bad);
        }
        Expr::Call(_, args, _) => args.iter().for_each(|(_, a)| go(a, good, bad)),
        Expr::Method(r, _, args, _) => {
            go(r, good, bad);
            args.iter().for_each(|a| go(a, good, bad));
        }
        Expr::Lambda(ps, b) => {
            ps.iter().for_each(|p| {
                bad.insert(p.clone());
            });
            go(b, good, bad);
        }
        Expr::Match(x, arms) => {
            go(x, good, bad);
            for (p, a) in arms {
                fs_pat(p, bad);
                go(a, good, bad);
            }
        }
        Expr::Block(stmts, tail) => {
            for s in stmts {
                match s {
                    // A string searched with `in` stays a string.
                    Stmt::Let { name: Pat::Bind(n), value: Expr::Str(_), .. } | Stmt::Assign { target: Expr::Ident(n), value: Expr::Str(_), .. } => {
                        bad.insert(n.clone());
                    }
                    Stmt::Let { name: Pat::Bind(_), value, .. } => go(value, good, bad),
                    Stmt::Let { name, value, .. } => {
                        fs_pat(name, bad);
                        go(value, good, bad);
                    }
                    Stmt::Assign { target: Expr::Ident(_), op: "=" | "+=", value, .. } => go(value, good, bad),
                    Stmt::Assign { target: t @ Expr::Field(b, _), op: "=" | "+=", value, .. } => {
                        if matches!(value, Expr::Str(_)) {
                            bad.insert(fs_key(t).unwrap());
                        }
                        go(b, good, bad);
                        go(value, good, bad);
                    }
                    Stmt::Assign { target, value, .. } => {
                        if let Some(r) = fs_root(target) {
                            bad.insert(r);
                        }
                        go(target, good, bad);
                        go(value, good, bad);
                    }
                    Stmt::Expr { expr, .. } | Stmt::Assert { cond: expr, .. } => go(expr, good, bad),
                    Stmt::Return { value: Some(v), .. } => go(v, good, bad),
                    _ => {}
                }
            }
            if let Some(t) = tail {
                go(t, good, bad);
            }
        }
    }
}
