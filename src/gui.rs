// Python tkinter programs, compiled onto the brevi_tk runtime (runtime/brevi_tk).
//
// A widget or tkinter variable is an int handle. The parser sends calls on them here before its own
// Python rewrites (`scan` finds the names that hold widgets), and `lower` turns them into calls of a few
// Rust functions (`tkx_call_s(w, "get", [args as json], [name, value, ...])` and friends) that hand the
// call to the runtime by name. Callbacks (`command=f`, `bind("<Key>", lambda e: ...)`, `after(500, tick)`)
// become ids: `tkx_dispatch(k, i, ev)` runs callback k, with the values a lambda captured (`lambda ch=ch:`)
// kept in typed lists, entry i.
use crate::ast::*;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

pub const WIDGETS: &[&str] = &[
    "Tk", "Toplevel", "Frame", "LabelFrame", "Labelframe", "Label", "Message", "Button", "Entry", "Text", "ScrolledText", "Checkbutton", "Radiobutton",
    "Listbox", "Scale", "Spinbox", "Canvas", "Scrollbar", "Combobox", "OptionMenu", "Progressbar", "Separator",
];
const VARS: &[&str] = &["StringVar", "IntVar", "BooleanVar", "DoubleVar"];
// Submodules a program names (`from tkinter import messagebox`).
const SUBS: &[&str] = &["messagebox", "ttk", "simpledialog", "filedialog", "font", "scrolledtext", "colorchooser", "constants"];

#[derive(Default, Clone)]
pub struct Reg {
    // alias -> submodule ("tk" for tkinter itself): `import tkinter as tk`, `from tkinter import ttk`.
    mods: HashMap<String, String>,
    // A name imported from tkinter: name -> "SUB.NAME" (`from tkinter import Button` is "tk.Button").
    bare: HashMap<String, String>,
    star: bool,
    // Names (and `self.` attributes) that hold a widget or variable -> its class.
    held: HashMap<String, String>,
    // Names holding lists or dicts of widgets.
    many: HashSet<String>,
}

thread_local! {
    static REG: RefCell<Option<Reg>> = const { RefCell::new(None) };
}

pub fn active() -> bool {
    REG.with(|r| r.borrow().is_some())
}

fn reg<T>(f: impl FnOnce(&Reg) -> T) -> Option<T> {
    REG.with(|r| r.borrow().as_ref().map(f))
}

fn ident_end(s: &str) -> usize {
    s.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(s.len())
}

// Finds the tkinter imports and the names assigned widgets. Clears the registry for a program without tkinter.
pub fn scan(src: &str) {
    let mut r = Reg::default();
    let mut any = false;
    for l in src.lines() {
        let t = l.trim();
        if let Some(rest) = t.strip_prefix("import ") {
            for part in rest.split(',') {
                let mut w = part.split_whitespace();
                let m = w.next().unwrap_or("");
                let alias = if w.next() == Some("as") { w.next().unwrap_or(m) } else { m };
                if m == "tkinter" || m == "Tkinter" {
                    any = true;
                    r.mods.insert(alias.to_string(), "tk".into());
                } else if let Some(sub) = m.strip_prefix("tkinter.") {
                    any = true;
                    r.mods.insert("tkinter".into(), "tk".into());
                    if alias != m {
                        r.mods.insert(alias.to_string(), sub.to_string());
                    }
                }
            }
        } else if let Some(rest) = t.strip_prefix("from ") {
            let Some((m, names)) = rest.split_once(" import ") else { continue };
            let m = m.trim();
            let sub = match m {
                "tkinter" | "Tkinter" => "tk",
                _ => match m.strip_prefix("tkinter.") {
                    Some(s) => s,
                    None => continue,
                },
            };
            any = true;
            for part in names.trim_matches(|c| c == '(' || c == ')' || c == ' ').split(',') {
                let mut w = part.split_whitespace();
                let n = w.next().unwrap_or("");
                let alias = if w.next() == Some("as") { w.next().unwrap_or(n) } else { n };
                if n == "*" {
                    r.star = true;
                } else if sub == "tk" && SUBS.contains(&n) {
                    r.mods.insert(alias.to_string(), n.to_string());
                } else if !n.is_empty() {
                    r.bare.insert(alias.to_string(), format!("{sub}.{n}"));
                }
            }
        }
    }
    if !any {
        REG.with(|x| *x.borrow_mut() = None);
        return;
    }
    // `NAME = tk.Button(...)`, `self.NAME = Label(...)`, `NAMES.append(tk.Button(...))`, `NAMES[k] = tk.Entry(...)`.
    let ctor = |r: &Reg, s: &str| -> Option<String> {
        let s = s.trim_start();
        let e = ident_end(s);
        let (head, rest) = s.split_at(e);
        let (class, after) = if let Some(rest) = rest.strip_prefix('.') {
            if !r.mods.contains_key(head) {
                return None;
            }
            let e2 = ident_end(rest);
            let (c, a) = rest.split_at(e2);
            // tkinter.ttk.Button
            if let Some(a2) = a.strip_prefix('.') {
                let e3 = ident_end(a2);
                (a2[..e3].to_string(), &a2[e3..])
            } else {
                (c.to_string(), a)
            }
        } else {
            let class = match r.bare.get(head) {
                Some(full) => full.rsplit('.').next().unwrap_or("").to_string(),
                None if r.star => head.to_string(),
                None => return None,
            };
            (class, rest)
        };
        if after.starts_with('(') && (WIDGETS.contains(&class.as_str()) || VARS.contains(&class.as_str())) {
            Some(class)
        } else {
            None
        }
    };
    for l in src.lines() {
        let t = l.trim();
        if let Some(i) = t.find(".append(") {
            let name = t[..i].trim_start_matches("self.");
            if ident_end(name) == name.len() && ctor(&r, &t[i + 8..]).is_some() {
                r.many.insert(name.to_string());
            }
        }
        let Some(eq) = t.find('=') else { continue };
        if t[eq + 1..].starts_with('=') || eq == 0 || matches!(t.as_bytes()[eq - 1], b'!' | b'<' | b'>' | b'+' | b'-' | b'*' | b'/') {
            continue;
        }
        let lhs = t[..eq].trim();
        let rhs = t[eq + 1..].trim();
        let lhs = lhs.strip_prefix("self.").unwrap_or(lhs);
        let (name, indexed) = match lhs.find('[') {
            Some(b) => (&lhs[..b], true),
            None => (lhs, false),
        };
        if name.is_empty() || ident_end(name) != name.len() {
            continue;
        }
        if let Some(c) = ctor(&r, rhs) {
            if indexed {
                r.many.insert(name.to_string());
            } else {
                r.held.insert(name.to_string(), c);
            }
        } else if rhs.starts_with('[') && rhs.contains(" for ") && ctor(&r, &rhs[1..]).is_some() {
            r.many.insert(name.to_string());
        }
    }
    REG.with(|x| *x.borrow_mut() = Some(r));
}

// Whether `e` is a widget or a tkinter variable.
fn is_gui(r: &Reg, e: &Expr) -> bool {
    match e {
        Expr::Ident(n) => r.held.contains_key(n),
        Expr::Field(b, n) => n == "widget" || (matches!(b.as_ref(), Expr::Ident(s) if s == "self" || s == "self_") && r.held.contains_key(n)),
        Expr::Index(b, _) => match b.as_ref() {
            Expr::Ident(n) => r.many.contains(n),
            Expr::Field(s, n) => matches!(s.as_ref(), Expr::Ident(s) if s == "self" || s == "self_") && r.many.contains(n),
            inner @ Expr::Index(..) => is_gui(r, inner),
            _ => false,
        },
        Expr::Call(n, _, _) => n.starts_with("tk:") && ctor_class(n).is_some(),
        _ => false,
    }
}

// The submodule an expression names: `tk`, `messagebox`, `tkinter.messagebox`.
fn module_of(r: &Reg, e: &Expr) -> Option<String> {
    match e {
        Expr::Ident(n) => r.mods.get(n).cloned(),
        Expr::Field(b, s) => match module_of(r, b) {
            Some(m) if m == "tk" && SUBS.contains(&s.as_str()) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn ctor_class(call: &str) -> Option<&str> {
    let c = call.strip_prefix("tk:")?.rsplit('.').next()?;
    (WIDGETS.contains(&c) || VARS.contains(&c)).then_some(c)
}

// The parser's hook for `recv.name(args)`: a call on tkinter (`tk.Label(...)`, `messagebox.showinfo(...)`)
// becomes `tk:SUB.name(args)`, a call on a widget `tk.name(recv, args)`. Keyword names stay.
pub fn method(recv: &Expr, name: &str, args: &[(Option<String>, Expr)], line: usize) -> Option<Expr> {
    reg(|r| {
        if let Some(m) = module_of(r, recv) {
            return Some(Expr::Call(format!("tk:{m}.{name}"), args.to_vec(), line));
        }
        if is_gui(r, recv) {
            let mut a = vec![(None, recv.clone())];
            a.extend(args.iter().cloned());
            return Some(Expr::Call(format!("tk.{name}"), a, line));
        }
        None
    })
    .flatten()
}

// The parser's hook for `Name(args)` with a name imported from tkinter (`Button(root, text="Hi")`).
pub fn bare_call(name: &str, args: &[(Option<String>, Expr)], line: usize) -> Option<Expr> {
    reg(|r| {
        let full = match r.bare.get(name) {
            Some(f) => f.clone(),
            None if r.star && (WIDGETS.contains(&name) || VARS.contains(&name) || name == "mainloop") => format!("tk.{name}"),
            None => return None,
        };
        Some(Expr::Call(format!("tk:{full}"), args.to_vec(), line))
    })
    .flatten()
}

// ------------------------------------------------------------------------------------------------ lowering

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Command, // no arguments
    Event,   // the event
    Trace,   // (name, index, mode)
}

struct Callback {
    // What runs: the body (a call or a lambda body), as statements, after the lets.
    lets: Vec<(String, Expr)>,
    body: Expr,
    caps: Vec<String>, // captured names, in the order of their lists
    self_ty: Option<String>, // the class of a captured `self_`
}

struct Lower<'a> {
    r: Reg,
    fns: HashSet<String>,
    params: HashMap<String, Vec<String>>,
    cbs: Vec<Callback>,
    // Names local to the function being lowered (None at top level, where names are globals).
    locals: Option<HashSet<String>>,
    sigs: &'a dyn Fn(&str) -> usize,
    errs: Vec<(usize, String)>,
    // The function being lowered (`App_build` for a method of class App).
    cur_fn: String,
    types: Vec<String>,
}

fn s(x: &str) -> Expr {
    Expr::Str(vec![StrPart::Lit(x.to_string())])
}

fn call(f: &str, args: Vec<Expr>, line: usize) -> Expr {
    Expr::Call(f.into(), args.into_iter().map(|a| (None, a)).collect(), line)
}

fn json(e: Expr) -> Expr {
    call("ax_to_json", vec![e], 0)
}

// tkinter's constants are their lower-case names (`tk.END` is "end", `tk.LEFT` "left").
fn constant(n: &str) -> Option<Expr> {
    (n.len() > 0 && n.chars().all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()) && n.chars().next().is_some_and(|c| c.is_ascii_uppercase())).then(|| s(&n.to_lowercase()))
}

const CB_OPTS: &[&str] = &["command", "postcommand", "validatecommand", "invalidcommand", "xscrollcommand", "yscrollcommand"];

// Names an expression reads.
fn reads(e: &Expr, out: &mut HashSet<String>) {
    let mut e = e.clone();
    crate::parser::walk_mut(&mut e, &mut |x| {
        if let Expr::Ident(n) = x {
            out.insert(n.clone());
        }
    });
}

fn assigned(e: &Expr, out: &mut HashSet<String>) {
    fn pat(p: &Pat, out: &mut HashSet<String>) {
        match p {
            Pat::Bind(n) => {
                out.insert(n.clone());
            }
            Pat::Tuple(ps) | Pat::Variant(_, ps) => ps.iter().for_each(|p| pat(p, out)),
            _ => {}
        }
    }
    let mut e = e.clone();
    crate::parser::walk_mut(&mut e, &mut |x| match x {
        Expr::Block(stmts, _) => {
            for st in stmts {
                match st {
                    Stmt::Let { name, .. } => pat(name, out),
                    Stmt::Assign { target: Expr::Ident(n), .. } => {
                        out.insert(n.clone());
                    }
                    _ => {}
                }
            }
        }
        Expr::For(p, _, _) => pat(p, out),
        _ => {}
    });
}

impl Lower<'_> {
    // The registration of a callback: an int expression, the callback's id at run time.
    fn callback(&mut self, e: Expr, mode: Mode, extra: Vec<Expr>, line: usize) -> Expr {
        let k = self.cbs.len() as i64;
        let (lets, body, caps, cap_vals): (Vec<(String, Expr)>, Expr, Vec<String>, Vec<Expr>) = match e {
            Expr::Ident(f) if self.fns.contains(&f) => {
                let n = (self.sigs)(&f).max(self.params.get(&f).map_or(0, |p| p.len()));
                let required = self.params.get(&f).map_or(0, |p| p.len());
                let _ = n;
                let mut args = vec![];
                let mut caps = vec![];
                let mut vals = vec![];
                for (j, x) in extra.into_iter().enumerate() {
                    let c = format!("a{j}_");
                    args.push(Expr::Ident(c.clone()));
                    caps.push(c);
                    vals.push(x);
                }
                if required > args.len() {
                    match mode {
                        Mode::Event => args.push(Expr::Ident("ev".into())),
                        Mode::Trace => {
                            for v in ["PY_VAR", "", "write"].iter().take(required - args.len()) {
                                args.push(s(v));
                            }
                        }
                        Mode::Command => {}
                    }
                }
                (vec![], call(&f, args, line), caps, vals)
            }
            Expr::Call(ref n, ref a, l) if n == "tk:lambda" => {
                // `lambda x, k=k: body`: names with defaults capture their value now.
                let mut lets = vec![];
                let mut caps = vec![];
                let mut vals = vec![];
                let mut body = Expr::Int(0);
                let mut plain = vec![];
                for (name, v) in a.iter() {
                    match (name, v) {
                        (Some(nm), v) => {
                            caps.push(nm.clone());
                            vals.push(v.clone());
                        }
                        (None, Expr::Lambda(ps, b)) => {
                            plain = ps.clone();
                            body = (**b).clone();
                        }
                        _ => {}
                    }
                }
                let _ = l;
                self.lambda_lets(&plain, mode, &mut lets);
                self.free_caps(&body, &plain, &mut caps, &mut vals);
                (lets, body, caps, vals)
            }
            Expr::Lambda(ps, b) => {
                let mut lets = vec![];
                self.lambda_lets(&ps, mode, &mut lets);
                let mut caps = vec![];
                let mut vals = vec![];
                self.free_caps(&b, &ps, &mut caps, &mut vals);
                (lets, *b, caps, vals)
            }
            // `command=self.on_click`: a method of the class whose method is being lowered, called on the
            // object (a shared handle, so the callback changes the object itself).
            Expr::Field(recv, m) if matches!(recv.as_ref(), Expr::Ident(x) if x == "self" || x == "self_") && self.method_fn(&m).is_some() => {
                let f = self.method_fn(&m).unwrap();
                let n = self.params.get(&f).map_or(1, |p| p.len()).saturating_sub(1);
                let me = Expr::Ident("self_".into());
                let lam = match mode {
                    Mode::Event if n >= 1 => Expr::Lambda(vec!["e_".into()], Box::new(call(&f, vec![me, Expr::Ident("e_".into())], line))),
                    Mode::Trace => Expr::Lambda(vec!["a_".into(), "b_".into(), "c_".into()], Box::new(call(&f, [me].into_iter().chain(["a_", "b_", "c_"].iter().take(n).map(|x| Expr::Ident(x.to_string()))).collect(), line))),
                    _ => {
                        // `after(ms, self.tick, n)`: the extra values are captured too.
                        let mut a = vec![me];
                        let mut caps = vec![];
                        for (j, x) in extra.iter().enumerate() {
                            let c = format!("x{j}_");
                            a.push(Expr::Ident(c.clone()));
                            caps.push((Some(c), x.clone()));
                        }
                        let lam = Expr::Lambda(vec![], Box::new(call(&f, a, line)));
                        if caps.is_empty() {
                            lam
                        } else {
                            caps.push((None, lam));
                            Expr::Call("tk:lambda".into(), caps, line)
                        }
                    }
                };
                return self.callback(lam, mode, vec![], line);
            }
            // `command=root.destroy`
            Expr::Field(recv, m) if reg_is_gui(&self.r, &recv) => {
                let body = self.lower_expr(Expr::Call(format!("tk.{m}"), vec![(None, *recv)], line));
                (vec![], body, vec![], vec![])
            }
            other => {
                self.errs.push((line, format!("a tkinter callback must be a function name or a lambda, found {}", short(&other))));
                return Expr::Int(0);
            }
        };
        let body = self.lower_expr(body);
        let caps_n = caps.len();
        let self_ty = if caps.iter().any(|c| c == "self_") { self.class_of_fn() } else { None };
        self.cbs.push(Callback { lets, body, caps, self_ty });
        if caps_n == 0 {
            call("tkx_cbreg", vec![Expr::Int(k), Expr::Int(0)], line)
        } else {
            call(&format!("tkx_mk{k}"), cap_vals.into_iter().map(|v| self.lower_expr(v)).collect(), line)
        }
    }

    // The function for `self.m` inside a method: `App_m` when lowering `App_build`.
    fn method_fn(&self, m: &str) -> Option<String> {
        let suffix = format!("_{m}");
        self.fns.iter().filter(|f| f.ends_with(&suffix)).find(|f| {
            let class = &f[..f.len() - suffix.len()];
            !class.is_empty() && self.cur_fn.starts_with(&format!("{class}_"))
        }).cloned()
    }

    // The class whose method is being lowered.
    fn class_of_fn(&self) -> Option<String> {
        self.types.iter().filter(|t| self.cur_fn.starts_with(&format!("{t}_"))).max_by_key(|t| t.len()).cloned()
    }

    fn lambda_lets(&self, ps: &[String], mode: Mode, lets: &mut Vec<(String, Expr)>) {
        match mode {
            Mode::Event => {
                if let Some(p) = ps.first() {
                    lets.push((p.clone(), Expr::Ident("ev".into())));
                }
            }
            Mode::Trace => {
                for (p, v) in ps.iter().zip(["PY_VAR", "", "write"]) {
                    lets.push((p.clone(), s(v)));
                }
            }
            Mode::Command => {}
        }
    }

    // A lambda inside a function reads that function's names: their values now are captured.
    fn free_caps(&self, body: &Expr, params: &[String], caps: &mut Vec<String>, vals: &mut Vec<Expr>) {
        let Some(locals) = &self.locals else { return };
        let mut rd = HashSet::new();
        reads(body, &mut rd);
        let mut inner = HashSet::new();
        assigned(body, &mut inner);
        let mut names: Vec<&String> = rd.iter().filter(|n| locals.contains(*n) && !params.contains(n) && !caps.contains(n) && !inner.contains(*n)).collect();
        names.sort();
        for n in names {
            caps.push(n.clone());
            vals.push(Expr::Ident(n.clone()));
        }
    }

    // Arguments of a call on tkinter: positional ones and `name, value` pairs, each as json.
    fn pack_args(&mut self, method: &str, args: Vec<(Option<String>, Expr)>, line: usize) -> (Expr, Expr) {
        let mut pos = vec![];
        let mut kw = vec![];
        // `after(500, lambda: f())`: the parser reads a lambda without parameters as a `lambda:` keyword.
        let args: Vec<(Option<String>, Expr)> = args.into_iter().map(|(n, a)| if n.as_deref() == Some("lambda") { (None, Expr::Lambda(vec![], Box::new(a))) } else { (n, a) }).collect();
        let (mut plain, named): (Vec<_>, Vec<_>) = args.into_iter().partition(|(n, _)| n.is_none() || n.as_deref() == Some("__star"));
        // Which positional argument is a callback, and what it is given.
        let cb = match method {
            "bind" | "bind_all" => Some((1, Mode::Event)),
            "tag_bind" => Some((2, Mode::Event)),
            "after" => Some((1, Mode::Command)),
            "after_idle" => Some((0, Mode::Command)),
            "trace_add" | "trace" | "trace_variable" => Some((1, Mode::Trace)),
            _ => None,
        };
        // `after(ms, f, x, y)`: f gets x and y.
        let extra: Vec<Expr> = match cb {
            Some((at, Mode::Command)) if plain.len() > at + 1 => plain.drain(at + 1..).map(|a| a.1).collect(),
            _ => vec![],
        };
        let mut extra = Some(extra);
        for (k, (_, a)) in plain.into_iter().enumerate() {
            let v = match cb {
                Some((at, mode)) if at == k => self.callback(a, mode, extra.take().unwrap_or_default(), line),
                _ => self.lower_expr(a),
            };
            pos.push(json(v));
        }
        for (name, a) in named {
            let n = name.unwrap_or_default();
            let v = if CB_OPTS.contains(&n.as_str()) { self.callback(a, Mode::Command, vec![], line) } else { self.lower_expr(a) };
            kw.push(json(s(&n)));
            kw.push(json(v));
        }
        (Expr::List(pos), Expr::List(kw))
    }

    fn class_of(&self, e: &Expr) -> Option<String> {
        match e {
            Expr::Ident(n) => self.r.held.get(n).cloned(),
            Expr::Field(_, n) => self.r.held.get(n).cloned(),
            Expr::Call(n, _, _) => ctor_class(n).map(String::from),
            _ => None,
        }
    }

    // The Rust function that returns what `method` returns (None: the call is a statement).
    fn typed(&self, class: Option<&str>, method: &str, nargs: usize) -> &'static str {
        match (class.unwrap_or(""), method) {
            ("StringVar", "get") => "tkx_call_s",
            ("IntVar", "get") => "tkx_call_i",
            ("BooleanVar", "get") => "tkx_call_b",
            ("DoubleVar", "get") => "tkx_call_f",
            ("Scale", "get") => "tkx_call_i",
            ("Listbox", "get") if nargs == 2 => "tkx_call_ls",
            (_, "get" | "cget" | "title" | "geometry" | "itemcget" | "askquestion" | "askstring" | "showinfo" | "showwarning" | "showerror") => "tkx_call_s",
            (_, "askyesno" | "askokcancel" | "askretrycancel" | "askyesnocancel" | "winfo_exists" | "instate") => "tkx_call_b",
            (_, "askfloat") => "tkx_call_f",
            (_, "curselection" | "find_all" | "find_overlapping" | "find_closest" | "find_withtag" | "bbox") => "tkx_call_li",
            (_, "coords") => "tkx_call_lf",
            (_, m) if m.starts_with("create_") || m.starts_with("winfo_") => "tkx_call_i",
            (_, "size" | "index" | "after" | "after_idle" | "askinteger" | "current" | "tkx_handle") => "tkx_call_i",
            _ => "tkx_call_s",
        }
    }

    fn lower_call(&mut self, name: String, mut args: Vec<(Option<String>, Expr)>, line: usize, stmt: bool) -> Expr {
        if name == "tk:lambda" {
            return Expr::Call(name, args, line);
        }
        if let Some(rest) = name.strip_prefix("tk:") {
            let (sub, f) = rest.split_once('.').unwrap_or(("tk", rest));
            if f == "Tk" {
                return call("tkx_root", vec![], line);
            }
            if WIDGETS.contains(&f) || VARS.contains(&f) {
                let (pos, kw) = self.pack_args(f, args, line);
                return call(if VARS.contains(&f) { "tkx_var" } else { "tkx_new" }, vec![s(f), pos, kw], line);
            }
            if f == "mainloop" {
                return call("tkx_call_n", vec![Expr::Int(0), s("mainloop"), Expr::List(vec![]), Expr::List(vec![])], line);
            }
            let (pos, kw) = self.pack_args(f, args, line);
            let t = if stmt { "tkx_call_n" } else { self.typed(None, f, 0) };
            return call(t, vec![Expr::Int(-1), s(&format!("{sub}.{f}")), pos, kw], line);
        }
        if let Some(m) = name.strip_prefix("tk.") {
            let recv = args.remove(0).1;
            let class = self.class_of(&recv);
            let nargs = args.iter().filter(|a| a.0.is_none()).count();
            let t = if stmt { "tkx_call_n" } else { self.typed(class.as_deref(), m, nargs) };
            let recv = self.lower_expr(recv);
            let (pos, kw) = self.pack_args(m, args, line);
            return call(t, vec![recv, s(m), pos, kw], line);
        }
        let args = args.into_iter().map(|(n, a)| (n, self.lower_expr(a))).collect();
        Expr::Call(name, args, line)
    }

    fn lower_expr(&mut self, e: Expr) -> Expr {
        self.lower(e, false)
    }

    fn lower(&mut self, e: Expr, stmt: bool) -> Expr {
        match e {
            Expr::Call(name, args, line) if name.starts_with("tk:") || name.starts_with("tk.") => self.lower_call(name, args, line, stmt),
            Expr::Field(b, n) if module_of(&self.r, &b).is_some() => constant(&n).unwrap_or(Expr::Field(b, n)),
            Expr::Ident(n) if (self.r.star || self.r.bare.contains_key(&n)) && constant(&n).is_some() && !self.fns.contains(&n) => {
                if self.r.star || self.r.bare.get(&n).is_some_and(|f| f.starts_with("tk.") || f.starts_with("constants.")) {
                    constant(&n).unwrap()
                } else {
                    Expr::Ident(n)
                }
            }
            // `e.char` (a keyword in Rust).
            Expr::Field(b, n) if n == "char" => Expr::Field(Box::new(self.lower_expr(*b)), "char_".into()),
            // `w["text"]`
            Expr::Index(b, k) if reg_is_gui(&self.r, &b) && matches!(k.as_ref(), Expr::Str(_)) => {
                let w = self.lower_expr(*b);
                call("tkx_call_s", vec![w, s("cget"), Expr::List(vec![json(*k)]), Expr::List(vec![])], 0)
            }
            Expr::Block(stmts, tail) => {
                let mut out = Vec::with_capacity(stmts.len());
                for st in stmts {
                    out.push(self.lower_stmt(st));
                }
                Expr::Block(out, tail.map(|t| Box::new(self.lower_expr(*t))))
            }
            Expr::Call(name, args, line) => Expr::Call(name, args.into_iter().map(|(n, a)| (n, self.lower_expr(a))).collect(), line),
            Expr::Method(r, m, args, line) => Expr::Method(Box::new(self.lower_expr(*r)), m, args.into_iter().map(|a| self.lower_expr(a)).collect(), line),
            Expr::List(xs) => Expr::List(xs.into_iter().map(|x| self.lower_expr(x)).collect()),
            Expr::Tuple(xs) => Expr::Tuple(xs.into_iter().map(|x| self.lower_expr(x)).collect()),
            Expr::Map(ps) => Expr::Map(ps.into_iter().map(|(k, v)| (self.lower_expr(k), self.lower_expr(v))).collect()),
            Expr::Str(parts) => Expr::Str(parts.into_iter().map(|p| match p {
                StrPart::Interp(x) => StrPart::Interp(self.lower_expr(x)),
                l => l,
            }).collect()),
            Expr::Unary(o, a) => Expr::Unary(o, Box::new(self.lower_expr(*a))),
            Expr::Binary(o, a, b) => Expr::Binary(o, Box::new(self.lower_expr(*a)), Box::new(self.lower_expr(*b))),
            Expr::Field(a, n) => Expr::Field(Box::new(self.lower_expr(*a)), n),
            Expr::TupleField(a, n) => Expr::TupleField(Box::new(self.lower_expr(*a)), n),
            Expr::Index(a, b) => Expr::Index(Box::new(self.lower_expr(*a)), Box::new(self.lower_expr(*b))),
            Expr::Lambda(ps, b) => Expr::Lambda(ps, Box::new(self.lower_expr(*b))),
            Expr::Try(a) => Expr::Try(Box::new(self.lower_expr(*a))),
            Expr::If(c, a, b) => Expr::If(Box::new(self.lower_expr(*c)), Box::new(self.lower(*a, stmt)), b.map(|b| Box::new(self.lower(*b, stmt)))),
            Expr::Match(x, arms) => Expr::Match(Box::new(self.lower_expr(*x)), arms.into_iter().map(|(p, a)| (p, self.lower(a, stmt))).collect()),
            Expr::For(p, a, b) => Expr::For(p, Box::new(self.lower_expr(*a)), Box::new(self.lower(*b, true))),
            Expr::While(a, b) => Expr::While(Box::new(self.lower_expr(*a)), Box::new(self.lower(*b, true))),
            Expr::Range(a, b) => Expr::Range(Box::new(self.lower_expr(*a)), Box::new(self.lower_expr(*b))),
            e => e,
        }
    }

    fn lower_stmt(&mut self, st: Stmt) -> Stmt {
        match st {
            Stmt::Expr { expr, line } => Stmt::Expr { expr: self.lower(expr, true), line },
            // `w["text"] = x` is `w.config(text=x)`.
            Stmt::Assign { target: Expr::Index(b, k), op: "=", value, line } if reg_is_gui(&self.r, &b) && matches!(k.as_ref(), Expr::Str(_)) => {
                let w = self.lower_expr(*b);
                let v = self.lower_expr(value);
                Stmt::Expr { expr: call("tkx_call_n", vec![w, s("config"), Expr::List(vec![]), Expr::List(vec![json(*k), json(v)])], line), line }
            }
            // `hist = None` for a name that holds a widget later: no widget yet.
            Stmt::Assign { target, op, value: Expr::Call(n, a, l), line } if n == "none" && a.is_empty() && reg_is_gui(&self.r, &target) => {
                let _ = l;
                Stmt::Assign { target, op, value: Expr::Int(-1), line }
            }
            Stmt::Let { name: Pat::Bind(nm), ty, value: Expr::Call(n, a, _), mutable, line } if n == "none" && a.is_empty() && self.r.held.contains_key(&nm) => {
                Stmt::Let { name: Pat::Bind(nm), ty, value: Expr::Int(-1), mutable, line }
            }
            Stmt::Let { name, ty, value, mutable, line } => Stmt::Let { name, ty, value: self.lower_expr(value), mutable, line },
            Stmt::Assign { target, op, value, line } => Stmt::Assign { target: self.lower_expr(target), op, value: self.lower_expr(value), line },
            Stmt::Return { value, line } => Stmt::Return { value: value.map(|v| self.lower_expr(v)), line },
            Stmt::Assert { cond, line } => Stmt::Assert { cond: self.lower_expr(cond), line },
            s => s,
        }
    }
}

fn reg_is_gui(r: &Reg, e: &Expr) -> bool {
    is_gui(r, e)
}

fn short(e: &Expr) -> String {
    let t = format!("{e:?}");
    t.chars().take(60).collect()
}

fn param(name: &str, ty: &str) -> Param {
    Param { name: name.into(), ty: Ty::Named(ty.into()), is_mut: false }
}

// The Rust functions the lowered calls use (bodies in GLUE).
fn rust_fn(name: &str, params: Vec<Param>, ret: Ty, code: &str) -> Item {
    Item::Fn { name: name.into(), params, ret, fallible: false, body: call("__rust__", vec![s(code)], 0), line: 0, script: false }
}

fn json_list() -> Ty {
    Ty::List(Box::new(Ty::Named("json".into())))
}

// Lowers a tkinter program: calls on widgets, callbacks, constants. Runs in the parser, before default
// arguments are filled in, on the program's functions and its top-level statements.
pub fn lower(items: &mut Vec<Item>, script: &mut Vec<Stmt>, sig_len: &dyn Fn(&str) -> usize) -> Result<(), crate::diag::Diag> {
    let Some(r) = reg(|r| r.clone()) else { return Ok(()) };
    let fns: HashSet<String> = items.iter().filter_map(|i| match i { Item::Fn { name, .. } => Some(name.clone()), _ => None }).collect();
    let params: HashMap<String, Vec<String>> = items.iter().filter_map(|i| match i { Item::Fn { name, params, .. } => Some((name.clone(), params.iter().map(|p| p.name.clone()).collect())), _ => None }).collect();
    let mut lw = Lower { r, fns, params, cbs: vec![], locals: None, sigs: sig_len, errs: vec![], cur_fn: String::new(), types: items.iter().filter_map(|i| match i { Item::Type { name, .. } => Some(name.clone()), _ => None }).collect() };
    for it in items.iter_mut() {
        if let Item::Fn { name, params, body, .. } = it {
            lw.cur_fn = name.clone();
            let mut loc: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
            assigned(body, &mut loc);
            lw.locals = Some(loc);
            let b = std::mem::replace(body, Expr::Int(0));
            *body = lw.lower(b, true);
        }
    }
    lw.locals = None;
    let st = std::mem::take(script);
    *script = st.into_iter().map(|s| lw.lower_stmt(s)).collect();
    if let Some((line, msg)) = lw.errs.first() {
        return Err(crate::diag::Diag::new("type", *line, 1, msg));
    }
    // Captured values: one global list per captured name, `tkx_mkK(...)` fills them and registers.
    let mut globals = vec![];
    let mut arms = vec![];
    for (k, cb) in lw.cbs.iter().enumerate() {
        let k = k as i64;
        let mut stmts = vec![];
        if !cb.caps.is_empty() {
            let mut mk = vec![];
            for (j, c) in cb.caps.iter().enumerate() {
                let g = format!("tkx_cap{k}_{j}");
                // A captured `self` is a handle to the object; its list is typed so nothing else has to infer it.
                let lty = match (&cb.self_ty, c.as_str()) {
                    (Some(t), "self_") => Some(Ty::List(Box::new(Ty::Named(t.clone())))),
                    _ => None,
                };
                globals.push(Stmt::Let { name: Pat::Bind(g.clone()), ty: lty, value: Expr::List(vec![]), mutable: true, line: 0 });
                mk.push(Stmt::Expr { expr: Expr::Method(Box::new(Expr::Ident(g.clone())), "push".into(), vec![Expr::Ident(format!("c{j}_"))], 0), line: 0 });
                stmts.push(Stmt::Let { name: Pat::Bind(c.clone()), ty: None, value: Expr::Index(Box::new(Expr::Ident(g)), Box::new(Expr::Ident("i".into()))), mutable: true, line: 0 });
            }
            let n = Expr::Binary("-", Box::new(Expr::Method(Box::new(Expr::Ident(format!("tkx_cap{k}_0"))), "len".into(), vec![], 0)), Box::new(Expr::Int(1)));
            mk.push(Stmt::Return { value: Some(call("tkx_cbreg", vec![Expr::Int(k), n], 0)), line: 0 });
            items.push(Item::Fn {
                name: format!("tkx_mk{k}"),
                params: cb.caps.iter().enumerate().map(|(j, c)| Param { name: format!("c{j}_"), ty: Ty::Named(match (&cb.self_ty, c.as_str()) { (Some(t), "self_") => t.clone(), _ => "?".into() }), is_mut: false }).collect(),
                ret: Ty::Named("?".into()),
                fallible: false,
                body: Expr::Block(mk, None),
                line: 0,
                script: false,
            });
        }
        for (n, v) in &cb.lets {
            stmts.push(Stmt::Let { name: Pat::Bind(n.clone()), ty: None, value: v.clone(), mutable: true, line: 0 });
        }
        stmts.push(Stmt::Expr { expr: cb.body.clone(), line: 0 });
        stmts.push(Stmt::Return { value: None, line: 0 });
        let cond = Expr::Binary("==", Box::new(Expr::Ident("k".into())), Box::new(Expr::Int(k)));
        arms.push(Stmt::Expr { expr: Expr::If(Box::new(cond), Box::new(Expr::Block(stmts, None)), None), line: 0 });
    }
    let mut g = globals;
    g.append(script);
    *script = g;
    items.push(Item::Type {
        name: "TkEvent".into(),
        fields: ["widget:int", "keysym:str", "char_:str", "x:int", "y:int", "delta:int", "num:int"]
            .iter()
            .map(|f| {
                let (n, t) = f.split_once(':').unwrap();
                Field { name: n.into(), ty: Ty::Named(t.into()), default: None }
            })
            .collect(),
        line: 0,
    });
    items.push(Item::Fn {
        name: "tkx_dispatch".into(),
        params: vec![param("k", "int"), param("i", "int"), param("ev", "TkEvent")],
        ret: Ty::Unit,
        fallible: false,
        body: Expr::Block(arms, None),
        line: 0,
        script: false,
    });
    let a4 = || vec![param("w", "int"), param("m", "str"), Param { name: "pos".into(), ty: json_list(), is_mut: false }, Param { name: "kw".into(), ty: json_list(), is_mut: false }];
    let named = |t: &str| Ty::Named(t.into());
    items.push(rust_fn("tkx_root", vec![], named("int"), "tkx_glue::root()"));
    items.push(rust_fn("tkx_new", vec![param("kind", "str"), Param { name: "pos".into(), ty: json_list(), is_mut: false }, Param { name: "kw".into(), ty: json_list(), is_mut: false }], named("int"), "tkx_glue::new(&kind, pos, kw)"));
    items.push(rust_fn("tkx_var", vec![param("kind", "str"), Param { name: "pos".into(), ty: json_list(), is_mut: false }, Param { name: "kw".into(), ty: json_list(), is_mut: false }], named("int"), "tkx_glue::var(&kind, pos, kw)"));
    items.push(rust_fn("tkx_cbreg", vec![param("k", "int"), param("i", "int")], named("int"), "tkx_glue::cbreg(k, i)"));
    items.push(rust_fn("tkx_call_n", a4(), Ty::Unit, "tkx_glue::call(w, &m, pos, kw);"));
    items.push(rust_fn("tkx_call_s", a4(), named("str"), "tkx_glue::call(w, &m, pos, kw).text()"));
    items.push(rust_fn("tkx_call_i", a4(), named("int"), "tkx_glue::call(w, &m, pos, kw).int()"));
    items.push(rust_fn("tkx_call_f", a4(), named("f64"), "tkx_glue::call(w, &m, pos, kw).float()"));
    items.push(rust_fn("tkx_call_b", a4(), named("bool"), "tkx_glue::call(w, &m, pos, kw).truthy()"));
    items.push(rust_fn("tkx_call_li", a4(), Ty::List(Box::new(named("int"))), "tkx_glue::list(tkx_glue::call(w, &m, pos, kw)).iter().map(|v| v.int()).collect()"));
    items.push(rust_fn("tkx_call_lf", a4(), Ty::List(Box::new(named("f64"))), "tkx_glue::list(tkx_glue::call(w, &m, pos, kw)).iter().map(|v| v.float()).collect()"));
    items.push(rust_fn("tkx_call_ls", a4(), Ty::List(Box::new(named("str"))), "tkx_glue::list(tkx_glue::call(w, &m, pos, kw)).iter().map(|v| v.text()).collect()"));
    Ok(())
}

// `brevi build --window`: the program draws a real window (through egui) instead of running headless.
pub static WINDOW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn window() -> bool {
    WINDOW.load(std::sync::atomic::Ordering::Relaxed)
}

// The crate a window build needs. From 0.37 eframe's default renderer is wgpu, which is a much bigger
// build and needs a GPU driver; Brevi keeps the OpenGL (glow) renderer, as in `runtime/brevi_tk/Cargo.toml`.
pub fn window_dep() -> crate::ffi::Dep {
    let features = ["-default", "accesskit", "clipboard", "default_fonts", "glow", "links", "wayland", "x11"];
    crate::ffi::Dep { name: "eframe".into(), version: "0.37".into(), features: features.iter().map(|f| f.to_string()).collect(), line: 0 }
}

// The runtime and the glue, pasted into the program as a `rust:` block would be. A window build also pastes
// the window front end; the runtime's `window` feature is then always on.
pub fn runtime() -> String {
    let lib = include_str!("../runtime/brevi_tk/src/lib.rs");
    let headless = include_str!("../runtime/brevi_tk/src/headless.rs");
    let win = include_str!("../runtime/brevi_tk/src/window.rs");
    let strip = |s: &str| s.lines().filter(|l| !l.trim_start().starts_with("//!")).collect::<Vec<_>>().join("\n");
    let module = |name: &str, src: &str| format!("mod {name} {{\n{}\n}}", strip(src).replace("use crate::*;", "use super::*;").replace("crate::headless", "super::headless"));
    let mut lib = strip(lib).replace("mod headless;", &module("headless", headless));
    if window() {
        lib = lib.replace("#[cfg(feature = \"window\")]\nmod window;", &module("window", win)).replace("#[cfg(feature = \"window\")]", "").replace("cfg!(feature = \"window\")", "true");
    } else {
        lib = lib.replace("#[cfg(feature = \"window\")]\nmod window;", "");
    }
    format!("#[allow(dead_code, unused)]\nmod brevi_tk {{\n{}\n}}\n{}\n", lib, GLUE)
}

const GLUE: &str = include_str!("gui_glue.rs");

#[cfg(test)]
mod tests {
    // `brevi build --window` must ask for the same eframe as the runtime crate is built and tested with.
    #[test]
    fn window_dep_matches_runtime_crate() {
        let manifest = include_str!("../runtime/brevi_tk/Cargo.toml");
        let line = manifest.lines().find(|l| l.starts_with("eframe = ")).expect("eframe line");
        let d = super::window_dep();
        assert!(line.contains(&format!("version = \"{}\"", d.version)), "{line}");
        assert!(line.contains("default-features = false") && d.features.iter().any(|f| f == "-default"));
        for f in d.features.iter().filter(|f| *f != "-default") {
            assert!(line.contains(&format!("\"{f}\"")), "feature {f} missing from {line}");
        }
        let toml = crate::ffi::cargo_toml("app", &[d]);
        assert!(toml.contains("eframe = { version = \"0.37\", default-features = false, features = [\"accesskit\""), "{toml}");
    }
}
