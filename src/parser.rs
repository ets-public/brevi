use std::collections::HashMap;
use crate::ast::*;
use crate::diag::Diag;
use crate::lexer::{lex, Tok, Token};

pub struct Parser {
    toks: Vec<Token>,
    pos: usize,
    // The function being parsed returns a map, so an arm `_ => {}` is an empty map.
    ret_map: bool,
    // Functions the program defines, which keep their own meaning over Python's len, sum, etc.
    user_fns: Vec<String>,
    // Functions written inside the function being parsed (Python's nested `def`), lifted to the top level.
    children: Vec<Item>,
    hoisted: Vec<Item>,
    // Statements that go right after the one just parsed (`a[i], b = x, y` is three).
    pending: Vec<Stmt>,
    // Names set from `set(...)` (Python sets, kept as sorted lists without repeats).
    set_names: Vec<String>,
    // A program written as Python: `/` divides exactly, as in Python 3.
    py: bool,
    // Functions with default values or `*args`: parameter names, defaults, and where `*args` is.
    sigs: HashMap<String, Sig>,
    // Class names in the program, and the names set from one of them (`c = Counter2()`), whose
    // method calls are the class's even when a built-in method has the same name.
    class_names: Vec<String>,
    held: Vec<String>,
    // The function being parsed has a `yield` (it builds its items in `ax_gen`).
    yields: bool,
    // `self.x: T = v` annotations met in the class being parsed.
    attr_tys: Vec<(String, Ty)>,
    // Python's `t = type(v)` in the function being parsed: `t is bool` reads as `type(v) is bool`.
    type_names: HashMap<String, Expr>,
    // Python error classes (`class Brk(Exception): pass`) and their bases, so `except Brk:` catches only Brk.
    exc_bases: HashMap<String, Vec<String>>,
    // Inside an except block: a bare `raise` raises the error being handled again.
    in_handler: usize,
    // Error classes with their own fields (`class RErr(Exception): def __init__(self, msg, line)`): real classes,
    // whose objects travel with the error (`except RErr as e: e.line`).
    exc_rich: Vec<String>,
    // Names set to `threading.Thread(...)`.
    threads: Vec<String>,
    // Parsing the body of a Python error class (`super().__init__(msg)` there does nothing).
    in_exc_class: bool,
    // What an error class passes up as its message: `super().__init__(msg)` gives Some(Some("msg")).
    exc_init_arg: Option<Option<String>>,
}

type Sig = (Vec<String>, Vec<Option<Expr>>, Option<usize>);

type R<T> = Result<T, Diag>;

// pybridge: values of Python libraries (see pybridge.rs). The program is parsed again while new names that
// hold them are found, so a use before the assignment (in an earlier function) counts.
pub fn parse_program(src: &str) -> R<Vec<Item>> {
    crate::pybridge::scan(src);
    let mut k = 0;
    loop {
        crate::pybridge::start_parse();
        let mut items = parse_once(src)?;
        k += 1;
        if !crate::pybridge::grew() || k >= 6 {
            crate::pybridge::lower(&mut items);
            return Ok(items);
        }
    }
}

fn parse_once(src: &str) -> R<Vec<Item>> {
    crate::gui::scan(src);
    let mut toks = crate::lexer::indent_blocks(lex(src)?);
    if crate::python_style(src) {
        print_aliases(&mut toks);
    }
    let user_fns = toks.windows(2).filter_map(|w| match (&w[0].tok, &w[1].tok) {
        (Tok::Kw("fn"), Tok::Ident(n)) => Some(n.clone()),
        _ => None,
    }).collect();
    let mut p = Parser { toks, pos: 0, ret_map: false, user_fns, children: vec![], hoisted: vec![], pending: vec![], set_names: vec![], py: false, sigs: HashMap::new(), class_names: vec![], held: vec![], yields: false, attr_tys: vec![], type_names: HashMap::new(), exc_bases: HashMap::new(), in_handler: 0, exc_rich: vec![], threads: vec![], in_exc_class: false, exc_init_arg: None };
    p.py = crate::python_style(src);
    // Python error classes, from every file, before any `except` that names one is read (the main file comes first).
    if p.py {
        let toks: Vec<Tok> = p.toks.iter().map(|t| t.tok.clone()).collect();
        let mut found: Vec<(String, Vec<String>, bool)> = vec![];
        let mut i = 0;
        while i + 3 < toks.len() {
            if let (Tok::Ident(c), Tok::Ident(n), Tok::Sym("(")) = (&toks[i], &toks[i + 1], &toks[i + 2]) {
                if c == "class" {
                    let mut j = i + 3;
                    let mut bases = vec![];
                    while j < toks.len() && toks[j] != Tok::Sym(")") {
                        if let Tok::Ident(b) = &toks[j] {
                            bases.push(b.clone());
                        }
                        j += 1;
                    }
                    // Its body: from the `{` after `:` to the matching `}`.
                    while j < toks.len() && toks[j] != Tok::Sym("{") {
                        j += 1;
                    }
                    let (mut depth, mut has_fn) = (0, false);
                    while j < toks.len() {
                        match &toks[j] {
                            Tok::Sym("{") => depth += 1,
                            Tok::Sym("}") => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            Tok::Kw("fn") => has_fn = true,
                            _ => {}
                        }
                        j += 1;
                    }
                    found.push((n.clone(), bases, has_fn));
                }
            }
            i += 1;
        }
        // Error classes and the classes below them, until nothing new is found.
        loop {
            let mut more = false;
            for (n, bases, has_fn) in &found {
                if p.exc_bases.contains_key(n) {
                    continue;
                }
                let errs: Vec<String> = bases.iter().filter(|b| b.ends_with("Exception") || b.ends_with("Error") || p.exc_bases.contains_key(*b)).cloned().collect();
                if !errs.is_empty() {
                    if *has_fn || errs.iter().any(|b| p.exc_rich.contains(b)) {
                        p.exc_rich.push(n.clone());
                    }
                    p.exc_bases.insert(n.clone(), errs);
                    more = true;
                }
            }
            if !more {
                break;
            }
        }
    }
    p.threads = p.toks.windows(5).filter_map(|w| match (&w[0].tok, &w[1].tok, &w[2].tok, &w[3].tok, &w[4].tok) {
        (Tok::Ident(t), Tok::Sym("="), Tok::Ident(m), Tok::Sym("."), Tok::Ident(c)) if m == "threading" && c == "Thread" => Some(t.clone()),
        _ => None,
    }).collect();
    p.class_names = p.toks.windows(2).filter_map(|w| match (&w[0].tok, &w[1].tok) {
        (Tok::Ident(c), Tok::Ident(n)) if c == "class" => Some(n.clone()),
        _ => None,
    }).collect();
    let mut items = Vec::new();
    let mut classes: Vec<(Item, Vec<String>)> = Vec::new();
    // Statements outside any function form the program's main (script mode).
    let mut script = Vec::new();
    let mut script_line = 0;
    loop {
        p.skip_newlines();
        if p.peek() == &Tok::Eof {
            break;
        }
        if matches!(p.peek(), Tok::Kw("fn" | "type" | "enum" | "test")) {
            items.push(p.item()?);
        } else if p.error_class() {
            continue;
        } else if matches!(p.peek(), Tok::Ident(w) if w == "class") && matches!(p.peek_at(1), Tok::Ident(_)) {
            let (ty, fns, bases) = p.class()?;
            classes.push((ty, bases));
            items.extend(fns);
        } else {
            if script.is_empty() {
                script_line = p.line();
            }
            script.push(p.stmt()?);
            script.append(&mut p.pending);
            if p.peek() != &Tok::Newline {
                return Err(p.err(&format!("expected end of line, found {}", tok_text(p.peek()))));
            }
        }
    }
    if p.py {
        fix_opens(&mut script);
    }
    if p.py {
        inline_fn_maps(&mut items, &mut script);
        fn_aliases(&mut items, &mut script);
    }
    // Python's `def` inside an if or loop outside any function: a plain top-level function
    // (what it reads from outside is a top-level name either way).
    items.append(&mut p.children);
    items.append(&mut p.hoisted);
    if !classes.is_empty() {
        inherit(&mut items, &mut classes);
        lower_methods(&mut items, &mut script, &classes);
        // Methods nothing calls (often a base class's, copied into its subclasses) are left out:
        // with no calls there is nothing to type their parameters from.
        let class_names: Vec<String> = classes.iter().filter_map(|(c, _)| match c { Item::Type { name, .. } => Some(format!("{name}_")), _ => None }).collect();
        loop {
            let text = format!("{items:?} {script:?}");
            let fn_names: Vec<String> = items.iter().filter_map(|i| match i { Item::Fn { name, .. } => Some(name.clone()), _ => None }).collect();
            let before = items.len();
            items.retain(|i| match i {
                Item::Fn { name, .. } if class_names.iter().any(|c| name.starts_with(c.as_str())) => {
                    // `board.add(...)` on a value whose class is only known once types are worked out (a parameter,
                    // or a class from another file) keeps the method, if its class is made somewhere.
                    let by_method = class_names.iter().filter(|c| name.starts_with(c.as_str())).any(|c| {
                        let m = &name[c.len()..];
                        let made = text.contains(&format!("Call(\"{c}new\"")) || !fn_names.contains(&format!("{c}new"));
                        // A tkinter callback names the method without calling it: `command=self.on_click`.
                        made && m != "new" && (text.contains(&format!("{m:?}, [")) || (crate::gui::active() && text.contains(&format!("Field(Ident(\"self_\"), {m:?})"))))
                    });
                    // `__str__` / `__repr__` are how the class prints.
                    let shows = name.ends_with("___str__") || name.ends_with("___repr__");
                    shows || by_method || text.contains(&format!("Call({name:?}")) || text.contains(&format!("Ident({name:?})"))
                }
                _ => true,
            });
            if items.len() == before {
                break;
            }
        }
        // So is a class nothing uses any more (a base class whose subclasses have their own copies).
        let text = format!("{items:?} {script:?}");
        let mut classes = classes;
        classes.retain(|(c, _)| match c {
            Item::Type { name, .. } => text.contains(&format!("Named({name:?})")) || text.contains(&format!("Call({name:?}")),
            _ => true,
        });
        items.extend(classes.into_iter().map(|c| c.0));
    }
    if crate::gui::active() {
        crate::gui::lower(&mut items, &mut script, &|f: &str| p.sigs.get(f).map_or(0, |s| s.0.len()))?;
    }
    if p.py {
        print_params(&mut items, &mut script, &mut p.sigs);
        stdout_params(&mut items, &mut script, &mut p.sigs);
        bound_callbacks(&mut items, &mut script);
    }
    if !p.sigs.is_empty() {
        fill_args(&mut items, &mut script, &p.sigs);
    }
    if p.py {
        none_flags(&mut items, &mut script);
    }
    if p.py && !script.is_empty() {
        // Top-level `sq = lambda x: x * x`, only ever called: Python's `def sq(x): return x * x`. Names a
        // function reads as a value, or that are already a function or type, stay lambdas.
        let mut skip: Vec<String> = vec![];
        for it in items.iter_mut() {
            match it {
                Item::Fn { name, body, .. } => {
                    skip.push(name.clone());
                    walk_mut(body, &mut |e| {
                        if let Expr::Ident(x) | Expr::Move(x) = e {
                            skip.push(x.clone());
                        }
                    });
                }
                Item::Type { name, .. } | Item::Enum { name, .. } => skip.push(name.clone()),
                _ => {}
            }
        }
        let mut body = Expr::Block(std::mem::take(&mut script), None);
        let mut kids: Vec<Item> = vec![];
        pick_names(&mut body);
        dict_with_base(&mut body);
        lambdas_to_defs(&mut body, &mut kids, script_line, &skip);
        let Expr::Block(st, _) = body else { unreachable!() };
        script = st;
        items.extend(kids);
    }
    tuples_as_lists(&mut items, &mut script);
    if !script.is_empty() {
        // Top-level statements beside fn main run first, then main, as in a Python file that ends with main().
        let mut script = script;
        if let Some(Item::Fn { name, params, .. }) = items.iter_mut().find(|i| matches!(i, Item::Fn { name, .. } if name == "main")) {
            if !params.is_empty() {
                return Err(Diag::new("parse", script_line, 1, "main takes no parameters").fix_hint("use args() to read command line arguments"));
            }
            *name = "main_".into();
            // `main()`, also inside `if __name__ == "__main__":`.
            let mut called = false;
            for st in script.iter_mut() {
                if let Stmt::Expr { expr, .. } = st {
                    called |= rename_main(expr);
                }
            }
            if !called {
                let line = script.last().map_or(script_line, |_| script_line);
                script.push(Stmt::Expr { expr: Expr::Call("main_".into(), vec![], line), line });
            }
        }
        items.push(Item::Fn {
            name: "main".into(),
            params: vec![],
            ret: Ty::Unit,
            fallible: true,
            body: Expr::Block(script, None),
            line: script_line,
            script: true,
        });
    }
    Ok(items)
}

fn tok_text(t: &Tok) -> String {
    match t {
        Tok::Int(n) => n.to_string(),
        Tok::Float(f) => f.to_string(),
        Tok::Str(s) => format!("\"{s}\""),
        Tok::Ident(s) => s.clone(),
        Tok::Kw(k) => k.to_string(),
        Tok::Sym(s) => s.to_string(),
        Tok::Newline => "end of line".into(),
        Tok::Eof => "end of file".into(),
    }
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }
    fn peek_at(&self, n: usize) -> &Tok {
        &self.toks[(self.pos + n).min(self.toks.len() - 1)].tok
    }
    fn line(&self) -> usize {
        self.toks[self.pos].line
    }
    fn next(&mut self) -> Tok {
        let t = self.toks[self.pos].tok.clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }
    fn err(&self, msg: &str) -> Diag {
        let t = &self.toks[self.pos];
        Diag::new("parse", t.line, t.col, msg)
    }
    fn is_sym(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Sym(x) if *x == s)
    }
    fn is_kw(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Kw(x) if *x == s)
    }
    fn eat_sym(&mut self, s: &str) -> bool {
        if self.is_sym(s) {
            self.next();
            true
        } else {
            false
        }
    }
    fn eat_kw(&mut self, s: &str) -> bool {
        if self.is_kw(s) {
            self.next();
            true
        } else {
            false
        }
    }
    fn expect_sym(&mut self, s: &str) -> R<()> {
        if self.eat_sym(s) {
            Ok(())
        } else {
            Err(self.err(&format!("expected '{s}', found {}", tok_text(self.peek()))).fix_hint(&format!("insert '{s}'")))
        }
    }
    fn ident(&mut self) -> R<String> {
        match self.next() {
            Tok::Ident(s) => Ok(s),
            t => {
                self.pos -= 1;
                Err(self.err(&format!("expected a name, found {}", tok_text(&t))))
            }
        }
    }
    fn skip_newlines(&mut self) {
        while self.peek() == &Tok::Newline {
            self.next();
        }
    }
    // Commas and newlines both separate list entries inside braces.
    fn skip_seps(&mut self) {
        while self.peek() == &Tok::Newline || self.is_sym(",") {
            self.next();
        }
    }

    fn item(&mut self) -> R<Item> {
        let line = self.line();
        if self.eat_kw("fn") {
            let name = self.ident()?;
            let name = if SHADOWABLE.contains(&name.as_str()) { format!("{name}_") } else { name };
            self.expect_sym("(")?;
            let mut params = Vec::new();
            let mut defaults: Vec<Option<Expr>> = Vec::new();
            let mut star = None;
            while !self.is_sym(")") {
                let is_mut = self.eat_kw("mut");
                // Python's `*args` collects the rest of the arguments in a list; a bare `*` only marks
                // keyword-only parameters, and `**kwargs` is not supported.
                if self.eat_sym("*") {
                    if self.is_sym(",") {
                        self.next();
                        continue;
                    }
                    star = Some(params.len());
                }
                let pname = self.ident()?;
                // Without a type, the parameter takes the type of what the calls pass (see infer::returns).
                let ty = if self.eat_sym(":") { self.ty()? } else { Ty::Named("?".into()) };
                let ty = if star == Some(params.len()) && !matches!(ty, Ty::Named(ref n) if n == "?") { Ty::List(Box::new(ty)) } else { ty };
                // Python's default values: `def f(x, n=1)`.
                defaults.push(if self.eat_sym("=") { Some(self.expr()?) } else { None });
                params.push(Param { name: pname, ty, is_mut });
                if !self.eat_sym(",") {
                    break;
                }
                self.skip_newlines();
            }
            self.expect_sym(")")?;
            if star.is_some() || defaults.iter().any(|d| d.is_some()) {
                self.sigs.insert(name.clone(), (params.iter().map(|p| p.name.clone()).collect(), defaults, star));
            }
            // Without `->` the return type is worked out from the body (see infer::returns).
            let ret = if self.eat_sym("->") { self.ty()? } else { Ty::Named("?".into()) };
            let fallible = self.eat_sym("!");
            self.ret_map = matches!(ret, Ty::Map(..));
            crate::pybridge::enter_fn(&name, &params);
            let saved_children = std::mem::take(&mut self.children);
            let saved_yields = std::mem::replace(&mut self.yields, false);
            let body = if self.eat_sym("=") {
                self.skip_newlines();
                self.expr()?
            } else if self.is_sym("{") || self.is_sym(":") {
                self.block()?
            } else {
                return Err(self.err("expected '=', ':' or '{' to start the function body"));
            };
            self.ret_map = false;
            crate::pybridge::leave_fn();
            let kids = std::mem::replace(&mut self.children, saved_children);
            let mut body = body;
            // `best: int` with no value only names a local's type; the local is made where it is set.
            walk_mut(&mut body, &mut |x| {
                if let Expr::Block(st, _) = x {
                    st.retain(|s| !matches!(s, Stmt::Let { value: Expr::Call(f, _, _), .. } if f == "__field"));
                }
            });
            if std::mem::replace(&mut self.yields, saved_yields) {
                body = generator_body(body, line);
            }
            // Python makes a default `[]` or `{}` once, so a function that changes it sees the earlier calls'
            // changes. Brevi makes it fresh at each call: refused.
            if self.py {
                if let Some((names, defaults, _)) = self.sigs.get(&name) {
                    for (pn, d) in names.iter().zip(defaults) {
                        let mutable = matches!(d, Some(Expr::List(_) | Expr::Map(_))) || matches!(d, Some(Expr::Call(f, _, _)) if f == "set" || f == "dict" || f == "list");
                        if !mutable {
                            continue;
                        }
                        let mut changed = false;
                        let mut b = body.clone();
                        walk_mut(&mut b, &mut |e| {
                            if let Expr::Method(r, m, _, _) = e {
                                if matches!(r.as_ref(), Expr::Ident(n) if n == pn) && ["push", "append", "pop", "insert", "remove", "clear", "extend", "update", "add", "discard", "setdefault", "sort", "reverse", "popleft", "appendleft"].contains(&m.as_str()) {
                                    changed = true;
                                }
                            }
                            if let Expr::Block(ss, _) = e {
                                for s in ss.iter() {
                                    match s {
                                        Stmt::Assign { target: Expr::Index(r, _), .. } if matches!(r.as_ref(), Expr::Ident(n) if n == pn) => changed = true,
                                        Stmt::Assign { target: Expr::Ident(n), op, .. } if n == pn && *op != "=" => changed = true,
                                        _ => {}
                                    }
                                }
                            }
                        });
                        if changed {
                            return Err(Diag::new("parse", line, 1, &format!("the default value of {pn} is made once in Python and kept between calls, so changing it inside the function changes it for later calls")).fix_hint(&format!("write {pn}=None and set {pn} = [] (or {{}}) inside the function when it is None")));
                        }
                    }
                }
            }
            let mut kids = kids;
            if self.py {
                pick_names(&mut body);
                dict_with_base(&mut body);
                lambdas_to_defs(&mut body, &mut kids, line, &[]);
                unbox_cells(&mut body, &mut kids);
                fall_off_none(&mut body, line);
            }
            if !kids.is_empty() {
                // A nested def's defaults and keywords are filled in before lifting adds the captured names.
                let ksigs: HashMap<String, Sig> = kids.iter().filter_map(|k| match k { Item::Fn { name: kn, .. } => self.sigs.get(kn).map(|s| (kn.clone(), s.clone())), _ => None }).collect();
                if !ksigs.is_empty() {
                    let mut st = vec![Stmt::Expr { expr: std::mem::replace(&mut body, Expr::Int(0)), line }];
                    fill_args(&mut kids, &mut st, &ksigs);
                    if let Some(Stmt::Expr { expr, .. }) = st.pop() {
                        body = expr;
                    }
                }
                let lifted = lift_nested(&name, &params, &mut body, kids);
                self.hoisted.extend(lifted);
            }
            return Ok(Item::Fn { name, params, ret, fallible, body, line, script: false });
        }
        if self.eat_kw("type") {
            let name = self.ident()?;
            self.expect_sym("{")?;
            let fields = self.fields("}")?;
            self.expect_sym("}")?;
            return Ok(Item::Type { name, fields, line });
        }
        if self.eat_kw("enum") {
            let name = self.ident()?;
            self.expect_sym("{")?;
            let mut variants = Vec::new();
            self.skip_seps();
            while !self.is_sym("}") {
                let vname = self.ident()?;
                let fields = if self.eat_sym("(") {
                    let f = self.fields(")")?;
                    self.expect_sym(")")?;
                    f
                } else {
                    Vec::new()
                };
                variants.push(Variant { name: vname, fields });
                self.skip_seps();
            }
            self.expect_sym("}")?;
            return Ok(Item::Enum { name, variants, line });
        }
        if self.eat_kw("test") {
            let name = match self.next() {
                Tok::Str(s) => s.replace("{{", "{").replace("}}", "}"),
                _ => return Err(self.err("expected a test name in quotes")),
            };
            let body = self.block()?;
            return Ok(Item::Test { name, body, line });
        }
        Err(self.err(&format!("expected fn, type, enum or test, found {}", tok_text(self.peek()))))
    }

    fn fields(&mut self, close: &str) -> R<Vec<Field>> {
        let mut fields = Vec::new();
        self.skip_seps();
        while !self.is_sym(close) {
            let name = self.ident()?;
            self.expect_sym(":")?;
            let ty = self.ty()?;
            let default = if self.eat_sym("=") { Some(self.expr()?) } else { None };
            fields.push(Field { name, ty, default });
            self.skip_seps();
        }
        Ok(fields)
    }

    // A type with an unknown part (`list[tuple]`, `dict[str, Any]`) is left whole to inference.
    fn ty(&mut self) -> R<Ty> {
        let t = self.ty_raw()?;
        let vague = |t: &Ty| matches!(t, Ty::Named(n) if n == "?");
        Ok(match &t {
            Ty::List(a) if vague(a) => Ty::Named("?".into()),
            Ty::Map(a, b) if vague(a) || vague(b) => Ty::Named("?".into()),
            Ty::Tuple(ts) if ts.iter().any(vague) => Ty::Named("?".into()),
            _ => t,
        })
    }

    fn ty_raw(&mut self) -> R<Ty> {
        // A quoted annotation (`-> "date | None"`) or `...` in `tuple[int, ...]` is left to inference.
        if matches!(self.peek(), Tok::Str(_)) {
            self.next();
            return Ok(Ty::Named("?".into()));
        }
        if self.eat_sym("..") {
            self.eat_sym(".");
            return Ok(Ty::Named("?".into()));
        }
        if self.eat_sym("[") {
            let t = self.ty()?;
            self.expect_sym("]")?;
            return Ok(Ty::List(Box::new(t)));
        }
        if self.eat_sym("{") {
            let k = self.ty()?;
            self.expect_sym(":")?;
            let v = self.ty()?;
            self.expect_sym("}")?;
            return Ok(Ty::Map(Box::new(k), Box::new(v)));
        }
        if self.eat_sym("(") {
            let mut ts = Vec::new();
            while !self.is_sym(")") {
                ts.push(self.ty()?);
                if !self.eat_sym(",") {
                    break;
                }
            }
            self.expect_sym(")")?;
            return Ok(if ts.is_empty() { Ty::Unit } else { Ty::Tuple(ts) });
        }
        let name = self.ident()?;
        if name == "heap" && self.eat_sym("[") {
            let t = self.ty()?;
            self.expect_sym("]")?;
            return Ok(Ty::Named(format!("AxHeap<{}>", crate::emit::rust_ty(&t))));
        }
        // Python's annotations: list[int], dict[str, int], set[str], tuple[int, str], Optional[T], and the
        // typing module's List, Dict, Set, Tuple. A bare `list` or `dict` leaves the item types to be worked out.
        let name = name.trim_start_matches("typing.").to_string();
        let ty = match name.as_str() {
            "float" => Ty::Named("f64".into()),
            "Decimal" | "Fraction" => Ty::Named("dec".into()),
            "list" | "List" | "set" | "Set" | "frozenset" | "deque" | "Sequence" | "Iterable" if self.is_sym("[") => {
                self.next();
                let t = self.ty()?;
                self.expect_sym("]")?;
                Ty::List(Box::new(t))
            }
            "dict" | "Dict" | "defaultdict" | "Mapping" | "OrderedDict" if self.is_sym("[") => {
                self.next();
                let k = self.ty()?;
                self.expect_sym(",")?;
                let v = self.ty()?;
                self.expect_sym("]")?;
                Ty::Map(Box::new(k), Box::new(v))
            }
            "tuple" | "Tuple" if self.is_sym("[") => {
                self.next();
                let mut ts = Vec::new();
                while !self.is_sym("]") {
                    ts.push(self.ty()?);
                    if !self.eat_sym(",") {
                        break;
                    }
                }
                self.expect_sym("]")?;
                Ty::Tuple(ts)
            }
            "Optional" if self.is_sym("[") => {
                self.next();
                let t = self.ty()?;
                self.expect_sym("]")?;
                t
            }
            // Union[int, None] is int; a union of real types is left to inference.
            "Union" if self.is_sym("[") => {
                self.next();
                let mut ts = vec![self.ty()?];
                while self.eat_sym(",") {
                    ts.push(self.ty()?);
                }
                self.expect_sym("]")?;
                ts.retain(|t| !matches!(t, Ty::Unit));
                if ts.len() == 1 { ts.remove(0) } else { Ty::Named("?".into()) }
            }
            "list" | "List" | "set" | "Set" | "dict" | "Dict" | "tuple" | "Tuple" | "Any" | "object" | "Callable" | "Iterator" | "Generator" => {
                // Callable[[int], str] and the like: skipped whole.
                if self.eat_sym("[") {
                    let mut depth = 1;
                    while depth > 0 && self.peek() != &Tok::Eof {
                        match self.next() {
                            Tok::Sym("[") => depth += 1,
                            Tok::Sym("]") => depth -= 1,
                            _ => {}
                        }
                    }
                }
                Ty::Named("?".into())
            }
            "None" => Ty::Unit,
            _ => Ty::Named(name),
        };
        // `int | None`: the None part is how a missing value is written; the value keeps its type.
        // `int | str` is left to inference.
        let mut ty = ty;
        while self.is_sym("|") && matches!(self.peek_at(1), Tok::Ident(_)) {
            if matches!(self.peek_at(1), Tok::Ident(n) if n == "None") {
                self.pos += 2;
                continue;
            }
            self.next();
            self.ty_raw()?;
            ty = Ty::Named("?".into());
        }
        Ok(ty)
    }

    fn block(&mut self) -> R<Expr> {
        // `if c: stmt` on one line.
        if self.eat_sym(":") {
            let st = self.stmt()?;
            if !self.pending.is_empty() {
                let mut v = vec![st];
                v.append(&mut self.pending);
                return Ok(Expr::Block(v, None));
            }
            return Ok(match st {
                Stmt::Expr { expr, .. } => Expr::Block(vec![], Some(Box::new(expr))),
                st => Expr::Block(vec![st], None),
            });
        }
        self.expect_sym("{")?;
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if self.eat_sym("}") {
                break;
            }
            if self.peek() == &Tok::Eof {
                return Err(self.err("block is missing its closing '}'").fix_hint("insert '}'"));
            }
            stmts.push(self.stmt()?);
            stmts.append(&mut self.pending);
            if self.is_sym("!") {
                return Err(self.err("unexpected '!' after an expression")
                    .fix_hint("use ? to pass an error up; ! only goes after a function's return type"));
            }
            if !(self.peek() == &Tok::Newline || self.is_sym("}")) {
                return Err(self.err(&format!("expected end of line, found {}", tok_text(self.peek())))
                    .fix_hint("put one statement per line"));
            }
        }
        if self.py {
            fix_opens(&mut stmts);
        }
        // The last expression statement is the block's value.
        let tail = match stmts.last() {
            Some(Stmt::Expr { .. }) => match stmts.pop() {
                Some(Stmt::Expr { expr, .. }) => Some(Box::new(expr)),
                _ => None,
            },
            _ => None,
        };
        Ok(Expr::Block(stmts, tail))
    }

    // Python's class: a type whose fields are the `self.x` it sets, `__init__` as the function `C_new`
    // (`C(a, b)` calls it), and each method `m` as the function `C_m(self, ...)`; `obj.m(x)` calls become
    // `C_m(obj, x)` in lower_methods. Field types are worked out like parameter types (infer::returns).
    fn class(&mut self) -> R<(Item, Vec<Item>, Vec<String>)> {
        let line = self.line();
        self.next();
        let name = self.ident()?;
        let mut bases = Vec::new();
        if self.eat_sym("(") {
            let mut depth = 1;
            while depth > 0 && self.peek() != &Tok::Eof {
                match self.next() {
                    Tok::Sym("(") => depth += 1,
                    Tok::Sym(")") => depth -= 1,
                    // `Exception` above a Python error class is not a class of the program.
                    Tok::Ident(b) if depth == 1 && !(self.exc_rich.contains(&name) && !self.class_names.contains(&b)) => bases.push(b),
                    _ => {}
                }
            }
        }
        let kids = self.children.len();
        let saved_attrs = std::mem::take(&mut self.attr_tys);
        self.in_exc_class = self.exc_rich.contains(&name);
        self.exc_init_arg = None;
        crate::pybridge::enter_class(&name);
        let body = self.block();
        crate::pybridge::leave_class();
        self.in_exc_class = false;
        let body = body?;
        let attr_tys = std::mem::replace(&mut self.attr_tys, saved_attrs);
        let mut methods: Vec<Item> = self.children.split_off(kids);
        // Python: `str(e)` of an error object is the message it was made with. Without its own `__str__`, an error
        // class gets one giving the field that holds that message, or one that stops the run if there is none.
        if self.exc_rich.contains(&name) && !methods.iter().any(|m| matches!(m, Item::Fn { name: n, .. } if n == "__str__" || n == "__repr__")) {
            let init = methods.iter().find_map(|m| match m {
                Item::Fn { name: n, params, body, .. } if n == "__init__" => Some((params.clone(), body.clone())),
                _ => None,
            });
            let msg_param = match (&self.exc_init_arg, &init) {
                (Some(arg), _) => arg.clone(),
                (None, Some((ps, _))) if ps.len() == 2 => Some(ps[1].name.clone()),
                _ => None,
            };
            let mut field = None;
            if let (Some(p), Some((_, b))) = (&msg_param, &init) {
                let mut b = b.clone();
                walk_mut(&mut b, &mut |e| {
                    if let Expr::Block(ss, _) = e {
                        for st in ss {
                            if let Stmt::Assign { target: Expr::Field(r, f), op: "=", value: Expr::Ident(x), .. } = st {
                                if *x == *p && matches!(r.as_ref(), Expr::Ident(s) if s == "self_") && field.is_none() {
                                    field = Some(f.clone());
                                }
                            }
                        }
                    }
                });
            }
            let value = match field {
                Some(f) => Expr::Call("str".into(), vec![(None, Expr::Field(Box::new(Expr::Ident("self_".into())), f))], line),
                None => Expr::Call("ax_unsupported".into(), vec![(None, Expr::Str(vec![StrPart::Lit(format!("str() of a {name} error is not supported yet"))]))], line),
            };
            methods.push(Item::Fn {
                name: "__str__".into(),
                params: vec![Param { name: "self_".into(), ty: Ty::Named("?".into()), is_mut: false }],
                ret: Ty::Named("?".into()),
                fallible: false,
                body: Expr::Block(vec![Stmt::Return { value: Some(value), line }], None),
                line,
                script: false,
            });
        }
        // Fields written in the class body, as a dataclass has them: `x: int`, `tags: list[str] = field(default_factory=list)`.
        let mut declared: Vec<Field> = Vec::new();
        if let Expr::Block(stmts, _) = &body {
            for st in stmts {
                if let Stmt::Let { name: Pat::Bind(n), ty, value, .. } = st {
                    let default = match value {
                        Expr::Call(f, _, _) if f == "__field" => None,
                        Expr::Call(f, a, _) if f == "field" => a.iter().find(|(k, _)| k.as_deref() == Some("default_factory")).map(|(_, v)| match v {
                            Expr::Ident(t) if t == "dict" => Expr::Map(vec![]),
                            Expr::Ident(t) if t == "list" || t == "set" => Expr::List(vec![]),
                            other => Expr::Call("str".into(), vec![(None, other.clone())], 0),
                        }).or_else(|| a.iter().find(|(k, _)| k.as_deref() == Some("default")).map(|(_, v)| v.clone())),
                        v => Some(v.clone()),
                    };
                    declared.push(Field { name: n.clone(), ty: ty.clone().unwrap_or(Ty::Named("?".into())), default });
                }
            }
        }
        // A dataclass prints as CPython's generated __repr__: its fields (a base dataclass's first), in order,
        // except those declared with field(repr=False).
        if let Some(py_name) = crate::lexer::dataclass_at(line) {
            let mut shown: Vec<String> = bases.iter().find_map(|b| dataclass(b)).map(|(_, f)| f).unwrap_or_default();
            if let Expr::Block(stmts, _) = &body {
                for st in stmts {
                    if let Stmt::Let { name: Pat::Bind(n), value, ty, .. } = st {
                        let hidden = matches!(value, Expr::Call(f, a, _) if f == "field" && a.iter().any(|(k, v)| k.as_deref() == Some("repr") && matches!(v, Expr::Bool(false))));
                        let class_var = matches!(ty, Some(Ty::Named(t)) if t.starts_with("ClassVar"));
                        if !hidden && !class_var && !shown.contains(n) {
                            shown.push(n.clone());
                        }
                    }
                }
            }
            DATACLASSES.with(|d| d.borrow_mut().insert(name.clone(), (py_name, shown)));
        }
        let mut fields: Vec<String> = declared.iter().map(|f| f.name.clone()).collect();
        let mut fns = Vec::new();
        for m in &methods {
            if let Item::Fn { body, .. } = m {
                let mut b = body.clone();
                walk_mut(&mut b, &mut |e| {
                    if let Expr::Block(stmts, _) = e {
                        for st in stmts.iter() {
                            if let Stmt::Assign { target: Expr::Field(r, f), .. } = st {
                                if matches!(r.as_ref(), Expr::Ident(n) if n == "self_") && !fields.contains(f) {
                                    fields.push(f.clone());
                                }
                            }
                            if let Stmt::Assign { target: Expr::Tuple(xs), .. } = st {
                                for x in xs {
                                    if let Expr::Field(r, f) = x {
                                        if matches!(r.as_ref(), Expr::Ident(n) if n == "self_") && !fields.contains(f) {
                                            fields.push(f.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
            }
        }
        let me = Ty::Named(name.clone());
        for m in methods {
            let Item::Fn { name: mname, mut params, ret, fallible, body, line, script } = m else { continue };
            // `def __len__(self)` is what `len(x)` calls.
            let mname = if mname == "__len__" { "len".to_string() } else { mname };
            // A method named like a built-in (`def args(self)`) keeps its name: it is only called on objects.
            let mname = match mname.strip_suffix('_') {
                Some(b) if SHADOWABLE.contains(&b) => b.to_string(),
                _ => mname,
            };
            if params.first().map_or(true, |p| p.name != "self_") {
                // A static method: a plain function.
                fns.push(Item::Fn { name: format!("{name}_{mname}"), params, ret, fallible, body, line, script });
                continue;
            }
            // Taken out, so a later class's method of the same name without defaults does not get this one's.
            if let Some(mut sig) = self.sigs.remove(&mname) {
                if mname == "__init__" {
                    sig.0.remove(0);
                    sig.1.remove(0);
                    sig.2 = sig.2.map(|k| k - 1);
                    self.sigs.insert(format!("{name}_new"), sig);
                } else {
                    self.sigs.insert(format!("{name}_{mname}"), sig);
                }
            }
            if mname == "__init__" {
                params.remove(0);
                let mut stmts = vec![Stmt::Let { name: Pat::Bind("self_".into()), ty: Some(me.clone()), value: Expr::Call("__default".into(), vec![], line), mutable: true, line }];
                match body {
                    Expr::Block(st, t) => {
                        stmts.extend(st);
                        if let Some(t) = t {
                            stmts.push(Stmt::Expr { expr: *t, line });
                        }
                    }
                    e => stmts.push(Stmt::Expr { expr: e, line }),
                }
                fns.push(Item::Fn { name: format!("{name}_new"), params, ret: Ty::Named("?".into()), fallible, body: Expr::Block(stmts, Some(Box::new(Expr::Ident("self_".into())))), line, script });
            } else {
                params[0].ty = me.clone();
                fns.push(Item::Fn { name: format!("{name}_{mname}"), params, ret, fallible, body, line, script });
            }
        }
        let fields = fields
            .into_iter()
            .map(|f| {
                let ty = attr_tys.iter().find(|(n, _)| *n == f).map(|(_, t)| t.clone()).unwrap_or(Ty::Named("?".into()));
                declared.iter().find(|d| d.name == f).cloned().unwrap_or(Field { name: f, ty, default: None })
            })
            .collect();
        Ok((Item::Type { name, fields, line }, fns, bases))
    }

    // Python's `class BadInput(Exception): pass`: Brevi errors are messages, so an error class needs nothing.
    // Skipped (true) when the class body holds only `pass`, `...` or a docstring.
    fn error_class(&mut self) -> bool {
        if !matches!(self.peek(), Tok::Ident(w) if w == "class") || !matches!(self.peek_at(1), Tok::Ident(_)) {
            return false;
        }
        let save = self.pos;
        let cname = match self.peek_at(1) {
            Tok::Ident(n) => n.clone(),
            _ => String::new(),
        };
        self.pos += 2;
        // `class BadInput(ValueError):` with any body is an error class too.
        let mut error_base = false;
        // `class Cat(Animal): pass` is a real class.
        let mut other_base = false;
        let mut bases = vec![];
        if self.eat_sym("(") {
            let mut depth = 1;
            while depth > 0 && self.peek() != &Tok::Eof {
                match self.next() {
                    Tok::Sym("(") => depth += 1,
                    Tok::Sym(")") => depth -= 1,
                    Tok::Ident(b) if depth == 1 && (b.ends_with("Exception") || b.ends_with("Error") || self.exc_bases.contains_key(&b)) => {
                        error_base = true;
                        bases.push(b);
                    }
                    Tok::Ident(b) if depth == 1 && b != "object" => other_base = true,
                    _ => {}
                }
            }
        }
        if error_base && !cname.is_empty() {
            self.exc_bases.insert(cname.clone(), bases.clone());
        }
        let kids = self.children.len();
        let body = self.block();
        let trivial = error_base || match body {
            Ok(Expr::Block(st, t)) => {
                st.iter().all(|s| matches!(s, Stmt::Expr { expr: Expr::Block(b, None), .. } if b.is_empty()) || matches!(s, Stmt::Expr { expr: Expr::Str(_), .. }))
                    && t.map_or(true, |t| matches!(*t, Expr::Str(_) | Expr::Block(..) | Expr::Range(..)))
            }
            _ => false,
        };
        // Python: an error class with methods of its own (or below one that has them) is a real class too.
        let rich = self.py && error_base && (self.children.len() > kids || bases.iter().any(|b| self.exc_rich.contains(b)));
        if rich && !self.exc_rich.contains(&cname) {
            self.exc_rich.push(cname.clone());
        }
        let trivial = (error_base && !rich) || (trivial && !other_base && self.children.len() == kids);
        self.children.truncate(kids);
        if !trivial {
            self.pos = save;
        }
        trivial
    }

    fn stmt(&mut self) -> R<Stmt> {
        let line = self.line();
        // Python's `del m[k]` and `del xs[i]`.
        if matches!(self.peek(), Tok::Ident(w) if w == "del") && !matches!(self.peek_at(1), Tok::Newline | Tok::Sym("=" | "(" | "." | "[")) {
            self.next();
            let e = self.expr()?;
            if let Some(d) = crate::pybridge::del(&e, line) {
                return Ok(d);
            }
            return match e {
                Expr::Index(b, k) => Ok(Stmt::Expr { expr: Expr::Method(b, "remove".into(), vec![*k], line), line }),
                // `del xs[a:]` and `del xs[a:b]` keep the items around the slice.
                Expr::Method(b, m, mut a, _) if m == "slice" && (a.len() == 1 || a.len() == 2) => {
                    let from = a.remove(0);
                    let mut value = Expr::Method(b.clone(), "slice".into(), vec![Expr::Int(0), from], line);
                    if let Some(to) = a.pop() {
                        value = Expr::Binary("+", Box::new(value), Box::new(Expr::Method(b.clone(), "slice".into(), vec![to], line)));
                    }
                    Ok(Stmt::Assign { target: *b, op: "=", value, line })
                }
                _ => Err(self.err("del takes m[k] or xs[i]").fix_hint("write del m[k]")),
            };
        }
        // Python's `with open(p) as f:` (also several, comma-separated): `f = open(p)`, the body, then `f.close()`,
        // all in the enclosing block (fix_opens then works on them like any opened file).
        if self.py && matches!(self.peek(), Tok::Ident(w) if w == "with") && matches!(self.peek_at(1), Tok::Ident(_)) {
            self.next();
            let mut opened = Vec::new();
            loop {
                let e = self.expr()?;
                // pybridge: `with lib_value as name:`.
                if opened.is_empty() && crate::pybridge::is_py(&e) {
                    let name = if matches!(self.peek(), Tok::Ident(w) if w == "as_" || w == "as") {
                        self.next();
                        Some(self.ident()?)
                    } else {
                        None
                    };
                    let body = self.block()?;
                    let mut stmts = crate::pybridge::with(e, name, body, self.pos, line);
                    let first = stmts.remove(0);
                    stmts.append(&mut self.pending);
                    self.pending = stmts;
                    return Ok(first);
                }
                if !matches!(&e, Expr::Call(f, _, _) if f.starts_with("__open_")) {
                    return Err(self.err("with is only supported with open(path) or open(path, mode)").fix_hint("read the file with open(path).read(), or use open(path, 'w') for writing"));
                }
                if !matches!(self.peek(), Tok::Ident(w) if w == "as_" || w == "as") {
                    return Err(self.err("expected 'as NAME' after open(...)").fix_hint("write with open(path) as f:"));
                }
                self.next();
                let name = self.ident()?;
                opened.push((name, e));
                if !self.eat_sym(",") {
                    break;
                }
            }
            let body = self.block()?;
            let (mut stmts, tail) = match body {
                Expr::Block(st, t) => (st, t),
                other => (vec![], Some(Box::new(other))),
            };
            if let Some(t) = tail {
                stmts.push(Stmt::Expr { expr: *t, line });
            }
            // A file being written is saved at the end of the body (before a final return); one being read needs nothing.
            let at = if matches!(stmts.last(), Some(Stmt::Return { .. })) { stmts.len() - 1 } else { stmts.len() };
            for (name, e) in opened.iter() {
                if !matches!(e, Expr::Call(k, _, _) if k == "__open_r") {
                    stmts.insert(at, Stmt::Expr { expr: Expr::Method(Box::new(Expr::Ident(name.clone())), "close".into(), vec![], line), line });
                }
            }
            let mut lets: Vec<Stmt> = opened.into_iter().map(|(name, e)| Stmt::Let { name: Pat::Bind(name), ty: None, value: e, mutable: true, line }).collect();
            let first = lets.remove(0);
            lets.extend(stmts);
            lets.append(&mut self.pending);
            self.pending = lets;
            return Ok(first);
        }
        // Python's `pass`.
        if matches!(self.peek(), Tok::Ident(w) if w == "pass") && matches!(self.peek_at(1), Tok::Newline | Tok::Sym("}")) {
            self.next();
            return Ok(Stmt::Expr { expr: Expr::Block(vec![], None), line });
        }
        // Python's nested `def`: lifted out of the function (see lift_nested).
        if self.is_kw("fn") && matches!(self.peek_at(1), Tok::Ident(_)) {
            let f = self.item()?;
            self.children.push(f);
            return Ok(Stmt::Expr { expr: Expr::Block(vec![], None), line });
        }
        // Python's `global count` / `nonlocal count`. A program that uses them gets Python's scope rule
        // exactly: a function that sets a top-level name without declaring it global has its own local.
        if matches!(self.peek(), Tok::Ident(w) if w == "global" || w == "nonlocal") && matches!(self.peek_at(1), Tok::Ident(_)) {
            self.next();
            let mut names = Vec::new();
            while let Tok::Ident(n) = self.peek().clone() {
                self.next();
                names.push((None, Expr::Ident(n)));
                if !self.eat_sym(",") {
                    break;
                }
            }
            return Ok(Stmt::Expr { expr: Expr::Call("__global".into(), names, line), line });
        }
        // Python's try/except/else/finally: `__try(body, handler, name)`. An error raised in body (a failing call or
        // `raise`) runs handler with `name` set to its message; else runs after a body without error, finally after both.
        if matches!(self.peek(), Tok::Ident(w) if w == "try_") && matches!(self.peek_at(1), Tok::Sym("{" | ":")) {
            self.next();
            let body = self.block()?;
            let mut handler: Option<Expr> = None;
            let mut clauses: Vec<(Vec<String>, Option<String>, Expr)> = vec![];
            let mut err_name = String::from("_");
            let mut els: Option<Expr> = None;
            let mut fin: Option<Expr> = None;
            loop {
                let save = self.pos;
                self.skip_newlines();
                if matches!(self.peek(), Tok::Ident(w) if w == "except") {
                    self.next();
                    let mut depth = 0;
                    let mut name = None;
                    let mut classes: Vec<String> = vec![];
                    let mut after_as = false;
                    while !(depth == 0 && matches!(self.peek(), Tok::Sym("{" | ":"))) && self.peek() != &Tok::Eof && self.peek() != &Tok::Newline {
                        match self.next() {
                            Tok::Sym("(" | "[") => depth += 1,
                            Tok::Sym(")" | "]") => depth -= 1,
                            Tok::Ident(w) if w == "as_" => {
                                after_as = true;
                                if let Tok::Ident(n) = self.peek().clone() {
                                    name = Some(n);
                                }
                            }
                            Tok::Ident(w) if !after_as => classes.push(w),
                            _ => {}
                        }
                    }
                    self.in_handler += 1;
                    let h = self.block();
                    self.in_handler -= 1;
                    let h = h?;
                    clauses.push((classes, name.clone(), h.clone()));
                    // Brevi errors carry only a message, so the first handler serves every error.
                    if handler.is_none() {
                        handler = Some(h);
                        if let Some(n) = name {
                            err_name = n;
                        }
                    }
                } else if self.is_kw("else") && handler.is_some() {
                    self.next();
                    els = Some(self.block()?);
                } else if matches!(self.peek(), Tok::Ident(w) if w == "finally") {
                    self.next();
                    fin = Some(self.block()?);
                } else {
                    self.pos = save;
                    break;
                }
            }
            let mut body = body;
            if let Some(Expr::Block(es, et)) = els {
                if let Expr::Block(bs, bt) = &mut body {
                    if let Some(t) = bt.take() {
                        bs.push(Stmt::Expr { expr: *t, line });
                    }
                    bs.extend(es);
                    *bt = et;
                }
            }
            // Python: several except clauses, or one naming an error class of the program, pick the handler by the
            // error's class (`except Brk: break` / `except Cont: continue`); an error no clause names goes on up.
            let user = |c: &String| self.exc_bases.contains_key(c);
            if self.py && (clauses.len() > 1 || clauses.iter().any(|(cs, _, _)| cs.iter().any(|c| user(c) || self.exc_rich.contains(c)))) {
                let mut chain: Option<Expr> = None;
                for (cs, nm, h) in clauses.iter().rev() {
                    let mut h = match h.clone() {
                        Expr::Block(st, t) => (st, t),
                        other => (vec![], Some(Box::new(other))),
                    };
                    if let Some(n) = nm {
                        // `except RErr as e`: the error's own object (fields and methods); else its message.
                        let (ty, value) = match cs.as_slice() {
                            [c] if self.exc_rich.contains(c) => (Some(Ty::Named(c.clone())), Expr::Call("__exc_get".into(), vec![], line)),
                            _ => (None, Expr::Call("__err".into(), vec![], line)),
                        };
                        h.0.insert(0, Stmt::Let { name: Pat::Bind(n.clone()), ty, value, mutable: false, line });
                    }
                    let h = Expr::Block(h.0, h.1);
                    let builtin = cs.is_empty() || cs.iter().any(|c| !user(c));
                    let names = if cs.is_empty() { "BaseException".to_string() } else { cs.join(",") };
                    let cond = Expr::Call("__exc_is".into(), vec![(None, Expr::Str(vec![StrPart::Lit(names)])), (None, Expr::Bool(builtin))], line);
                    let rest = chain.take().unwrap_or_else(|| Expr::Call("fail".into(), vec![(None, Expr::Call("__err_raw".into(), vec![], line))], line));
                    chain = Some(Expr::If(Box::new(cond), Box::new(h), Some(Box::new(Expr::Block(vec![], Some(Box::new(rest)))))));
                }
                handler = chain.map(|c| Expr::Block(vec![], Some(Box::new(c))));
                err_name = "_".into();
            }
            // An if whose condition is `__try()`: every pass over branches (types, names set on some paths) applies as is.
            let mut handler = match handler {
                Some(Expr::Block(st, t)) => (st, t),
                Some(other) => (vec![], Some(Box::new(other))),
                None => (vec![], None),
            };
            if err_name != "_" {
                handler.0.insert(0, Stmt::Let { name: Pat::Bind(err_name), ty: None, value: Expr::Call("__err".into(), vec![], line), mutable: false, line });
            }
            let handler = Expr::Block(handler.0, handler.1);
            let t = Expr::If(Box::new(Expr::Call("__try".into(), vec![], line)), Box::new(body), Some(Box::new(handler)));
            return Ok(match fin {
                Some(Expr::Block(mut fs, ft)) => {
                    fs.insert(0, Stmt::Expr { expr: t, line });
                    Stmt::Expr { expr: Expr::Block(fs, ft), line }
                }
                _ => Stmt::Expr { expr: t, line },
            });
        }
        // Python's `raise ValueError("bad")` is fail("bad"); a bare `raise` or `raise KeyError` fails with the name.
        if matches!(self.peek(), Tok::Ident(w) if w == "raise") {
            self.next();
            let msg = if (self.peek() == &Tok::Newline || self.is_sym("}")) && self.py && self.in_handler > 0 {
                // A bare `raise` in an except block: the same error again.
                Expr::Call("__err_raw".into(), vec![], line)
            } else if self.peek() == &Tok::Newline || self.is_sym("}") {
                Expr::Str(vec![StrPart::Lit("error".into())])
            } else {
                let e = self.expr()?;
                if let Tok::Ident(w) = self.peek().clone() {
                    if w == "from" {
                        self.next();
                        self.expr()?;
                    }
                }
                // `raise SystemExit(3)` ends the program like sys.exit(3).
                match &e {
                    Expr::Call(n, args, _) if self.py && n == "SystemExit" && args.len() <= 1 && args.iter().all(|a| a.0.is_none()) => {
                        let code = args.first().map_or(Expr::Int(0), |a| a.1.clone());
                        return Ok(Stmt::Expr { expr: Expr::Method(Box::new(code), "exit_with".into(), vec![], line), line });
                    }
                    Expr::Ident(n) if self.py && n == "SystemExit" => {
                        return Ok(Stmt::Expr { expr: Expr::Method(Box::new(Expr::Int(0)), "exit_with".into(), vec![], line), line });
                    }
                    _ => {}
                }
                let class = match &e {
                    Expr::Call(n, _, _) | Expr::Ident(n) if n.chars().next().map_or(false, |c| c.is_uppercase()) => Some(n.clone()),
                    _ => None,
                };
                let raised = match &e {
                    Expr::Call(..) => Some(e.clone()),
                    Expr::Ident(n) if self.exc_rich.contains(n) => Some(Expr::Call(n.clone(), vec![], line)),
                    _ => None,
                };
                let msg = match e {
                    Expr::Call(n, mut args, _) if n.chars().next().map_or(false, |c| c.is_uppercase()) => {
                        if args.is_empty() { Expr::Str(vec![StrPart::Lit(n)]) } else { Expr::Call("str".into(), vec![(None, args.remove(0).1)], line) }
                    }
                    Expr::Ident(n) => Expr::Str(vec![StrPart::Lit(n)]),
                    other => Expr::Call("str".into(), vec![(None, other)], line),
                };
                // Python: the error carries its class and the classes above it, for `except Base:`.
                match class {
                    Some(c) if self.py => {
                        let rich = self.exc_rich.contains(&c);
                        let mut chain = vec![c.clone()];
                        let mut at = c;
                        while let Some(b) = self.exc_bases.get(&at).and_then(|b| b.first()).cloned() {
                            if chain.contains(&b) {
                                break;
                            }
                            chain.push(b.clone());
                            at = b;
                        }
                        match raised {
                            Some(obj) if rich => Expr::Call("__exc_obj".into(), vec![(None, Expr::Str(vec![StrPart::Lit(chain.join(","))])), (None, msg), (None, obj)], line),
                            _ => Expr::Call("__exc".into(), vec![(None, Expr::Str(vec![StrPart::Lit(chain.join(","))])), (None, msg)], line),
                        }
                    }
                    _ => msg,
                }
            };
            return Ok(Stmt::Expr { expr: Expr::Call("fail".into(), vec![(None, msg)], line), line });
        }
        if self.is_kw("let") || self.is_kw("var") {
            let mutable = self.next() == Tok::Kw("var");
            let name = bind_upper(self.pat()?);
            let ty = if self.eat_sym(":") { Some(self.ty()?) } else { None };
            self.expect_sym("=")?;
            self.skip_newlines();
            let value = self.expr()?;
            return Ok(Stmt::Let { name, ty, value, mutable, line });
        }
        // `x: T = v` declares a typed variable, like `var x: T = v`.
        if matches!(self.peek(), Tok::Ident(_)) && matches!(self.peek_at(1), Tok::Sym(":")) {
            let name = bind_upper(self.pat()?);
            self.expect_sym(":")?;
            // `c: list = [0] * 10`: a type with unknown parts is worked out from the value.
            let ty = Some(self.ty()?).filter(|t| !matches!(t, Ty::Named(n) if n == "?"));
            // A dataclass field without a default (`x: int`).
            if self.peek() == &Tok::Newline || self.is_sym("}") {
                return Ok(Stmt::Let { name, ty, value: Expr::Call("__field".into(), vec![], line), mutable: true, line });
            }
            self.expect_sym("=")?;
            self.skip_newlines();
            let value = self.expr()?;
            return Ok(Stmt::Let { name, ty, value, mutable: true, line });
        }
        // A generator collects what it yields in a list, which it returns (see item()).
        // (`yield [x for x in xs]` yields a list; only a Brevi name `yield_` is indexed.)
        if matches!(self.peek(), Tok::Ident(w) if w == "yield_") && !matches!(self.peek_at(1), Tok::Sym("=" | "." | ")" | ",")) && (self.py || !matches!(self.peek_at(1), Tok::Sym("["))) {
            self.next();
            self.yields = true;
            let acc = Expr::Ident("ax_gen".into());
            let e = if matches!(self.peek(), Tok::Ident(w) if w == "from") {
                self.next();
                return Ok(Stmt::Assign { target: acc, op: "+=", value: self.expr()?, line });
            } else {
                let mut v = self.expr()?;
                if self.is_sym(",") {
                    let mut xs = vec![v];
                    while self.eat_sym(",") {
                        xs.push(self.expr()?);
                    }
                    v = Expr::Tuple(xs);
                }
                Expr::Method(Box::new(acc), "push".into(), vec![v], line)
            };
            return Ok(Stmt::Expr { expr: e, line });
        }
        if self.eat_kw("return") {
            let mut value = if self.peek() == &Tok::Newline || self.is_sym("}") { None } else { Some(self.expr()?) };
            // Python's `return a, b` returns a tuple.
            if value.is_some() && self.is_sym(",") {
                let mut xs = vec![value.take().unwrap()];
                while self.eat_sym(",") {
                    xs.push(self.expr()?);
                }
                value = Some(Expr::Tuple(xs));
            }
            if let Some(v) = &value {
                crate::pybridge::note_return(v);
            }
            return Ok(Stmt::Return { value, line });
        }
        if self.eat_kw("break") {
            return Ok(Stmt::Break { line });
        }
        if self.eat_kw("continue") {
            return Ok(Stmt::Continue { line });
        }
        if self.eat_kw("assert") {
            return Ok(Stmt::Assert { cond: self.expr()?, line });
        }
        let mut e = self.expr()?;
        // Python's `xs[a:b] = ys` replaces those items in place (`xs[:k] = xs[k-1::-1]`).
        if let Expr::Method(r, m, a, l) = &e {
            if self.py && m == "slice" && (a.len() == 2 || a.len() == 1 && !matches!(a.as_slice(), [Expr::Int(0)])) && self.is_sym("=") {
                self.next();
                self.skip_newlines();
                let value = self.expr()?;
                let mut args = a.clone();
                if args.len() == 1 {
                    args.push(Expr::Int(i64::MAX));
                }
                args.push(value);
                return Ok(Stmt::Expr { expr: Expr::Method(r.clone(), "splice".into(), args, *l), line });
            }
        }
        // `lots[:] = [...]` replaces the list's items in place (the caller sees them).
        if let Expr::Method(r, m, a, _) = &e {
            if m == "slice" && matches!(a.as_slice(), [Expr::Int(0)]) && self.is_sym("=") {
                self.next();
                self.skip_newlines();
                let value = self.expr()?;
                let tmp = format!("ax_new{}", self.pos);
                let r = (**r).clone();
                self.pending.push(Stmt::Expr { expr: Expr::Method(Box::new(r.clone()), "clear".into(), vec![], line), line });
                let x = Expr::Ident("ax_x".into());
                let push = Stmt::Expr { expr: Expr::Method(Box::new(r), "push".into(), vec![x], line), line };
                self.pending.push(Stmt::Expr { expr: Expr::For(Pat::Bind("ax_x".into()), Box::new(Expr::Ident(tmp.clone())), Box::new(Expr::Block(vec![push], None))), line });
                return Ok(Stmt::Assign { target: Expr::Ident(tmp), op: "=", value, line });
            }
        }
        // `self.names: list[str] = []`: the field's type comes from what is put in it, so the annotation is read
        // and dropped.
        if matches!(e, Expr::Field(..) | Expr::Index(..)) && self.is_sym(":") {
            self.next();
            let ty = self.ty()?;
            if let Expr::Field(r, f) = &e {
                if matches!(r.as_ref(), Expr::Ident(n) if n == "self_") && !matches!(&ty, Ty::Named(n) if n == "?") {
                    self.attr_tys.push((f.clone(), ty));
                }
            }
            self.expect_sym("=")?;
            self.skip_newlines();
            let value = self.expr()?;
            return Ok(Stmt::Assign { target: e, op: "=", value, line });
        }
        // `a, b = ...` assigns a tuple, like `(a, b) = ...`.
        let mut bare = false;
        if matches!(e, Expr::Ident(_) | Expr::Tuple(_)) && self.is_sym(",") && self.bare_targets() {
            let mut xs = vec![e];
            while self.eat_sym(",") {
                xs.push(self.postfix()?);
            }
            e = Expr::Tuple(xs);
            bare = true;
        } else if matches!(e, Expr::Ident(_) | Expr::Index(..) | Expr::Field(..)) && self.is_sym(",") {
            // `st[-1], st[-2] = st[-2], st[-1]`: the values are worked out first, then set one by one.
            let save = self.pos;
            let mut xs = vec![e.clone()];
            let mut ok = true;
            while self.eat_sym(",") {
                match self.postfix() {
                    Ok(t @ (Expr::Ident(_) | Expr::Index(..) | Expr::Field(..))) => xs.push(t),
                    _ => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok && self.eat_sym("=") {
                self.skip_newlines();
                let mut vs = vec![self.expr()?];
                while self.eat_sym(",") {
                    vs.push(self.expr()?);
                }
                if vs.len() != xs.len() {
                    return Err(self.err(&format!("{} names but {} values", xs.len(), vs.len())));
                }
                let tmp = format!("ax_sw{}", self.pos);
                let mut stmts = vec![Stmt::Let { name: Pat::Bind(tmp.clone()), ty: None, value: Expr::Tuple(vs), mutable: false, line }];
                for (k, t) in xs.into_iter().enumerate() {
                    stmts.push(Stmt::Assign { target: t, op: "=", value: Expr::TupleField(Box::new(Expr::Ident(tmp.clone())), k), line });
                }
                let first = stmts.remove(0);
                self.pending.extend(stmts);
                return Ok(first);
            }
            self.pos = save;
        }
        // Python's `x |= m`, `x //= 2`, `x <<= 1` and the like: `x = x | m`.
        for op in ["//=", "%=", "|=", "&=", "^=", "<<=", ">>="] {
            if self.eat_sym(op) {
                self.skip_newlines();
                let value = self.expr()?;
                let bin: &'static str = match op {
                    "//=" => "//",
                    "%=" => "%",
                    "|=" => "|",
                    "&=" => "&",
                    "^=" => "^",
                    "<<=" => "<<",
                    _ => ">>",
                };
                return Ok(Stmt::Assign { target: e.clone(), op: "=", value: Expr::Binary(bin, Box::new(e), Box::new(value)), line });
            }
        }
        for op in ["=", "+=", "-=", "*=", "/="] {
            if self.eat_sym(op) {
                let op: &'static str = match op {
                    "=" => "=",
                    "+=" => "+=",
                    "-=" => "-=",
                    "*=" => "*=",
                    _ => "/=",
                };
                self.skip_newlines();
                let mut value = self.expr()?;
                // Python's `a = b = 0` sets each name.
                if op == "=" && matches!(e, Expr::Ident(_)) && matches!(value, Expr::Ident(_)) && self.is_sym("=") {
                    let mut names = vec![e.clone(), value];
                    while self.eat_sym("=") {
                        names.push(self.expr()?);
                    }
                    let v = names.pop().unwrap();
                    // A plain value (`pl = pd = None`, `a = b = 0`) is set name by name, so each is typed alone.
                    let plain = matches!(&v, Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Str(_) | Expr::Ident(_))
                        || matches!(&v, Expr::Call(f, a, _) if f == "none" && a.is_empty())
                        || matches!(&v, Expr::List(x) if x.is_empty())
                        || matches!(&v, Expr::Map(x) if x.is_empty());
                    if plain && names.iter().all(|n| matches!(n, Expr::Ident(_))) {
                        let first = names.remove(0);
                        for n in names {
                            self.pending.push(Stmt::Assign { target: n, op, value: v.clone(), line });
                        }
                        return Ok(Stmt::Assign { target: first, op, value: v, line });
                    }
                    if names.iter().all(|n| matches!(n, Expr::Ident(_))) {
                        let vs = vec![v; names.len()];
                        return Ok(Stmt::Assign { target: Expr::Tuple(names), op, value: Expr::Tuple(vs), line });
                    }
                    return Err(self.err("expected names before each ="));
                }
                if bare && self.is_sym(",") {
                    let mut vs = vec![value];
                    while self.eat_sym(",") {
                        vs.push(self.expr()?);
                    }
                    value = Expr::Tuple(vs);
                }
                crate::pybridge::note_assign(&e, &value);
                if let (Expr::Ident(n), Expr::Call(c, _, _)) = (&e, &value) {
                    if self.class_names.contains(c) {
                        self.held.push(n.clone());
                    }
                }
                // `s = set(ans)`: s is a set, so `s <= other` is a subset test.
                if let Expr::Ident(n) = &e {
                    if op == "=" && Self::set_made(&value) {
                        self.set_names.push(n.clone());
                    }
                }
                let e = if op == "=" && matches!(&e, Expr::Tuple(xs) if xs.iter().any(|x| matches!(x, Expr::Tuple(_)))) { self.flat_target(e, line) } else { e };
                // `order, questions = [], {}`: one name at a time, so each empty value's type is worked out alone.
                if let (Expr::Tuple(ns), "=", Expr::Tuple(vs)) = (&e, op, &value) {
                    let lit = |v: &Expr| matches!(v, Expr::List(x) if x.is_empty()) || matches!(v, Expr::Map(x) if x.is_empty()) || matches!(v, Expr::Int(_) | Expr::Float(_) | Expr::Bool(_)) || matches!(v, Expr::Call(f, a, _) if f == "none" && a.is_empty());
                    if self.py && ns.len() == vs.len() && ns.iter().all(|n| matches!(n, Expr::Ident(_))) && vs.iter().all(lit) && vs.iter().any(|v| matches!(v, Expr::List(_) | Expr::Map(_))) {
                        let mut pairs = ns.iter().cloned().zip(vs.iter().cloned());
                        let (n0, v0) = pairs.next().unwrap();
                        for (n, v) in pairs {
                            self.pending.push(Stmt::Assign { target: n, op, value: v, line });
                        }
                        return Ok(Stmt::Assign { target: n0, op, value: v0, line });
                    }
                }
                // `m.setdefault(k, {})[j] = v`: set m[k] when missing, then m[k][j] = v (k is read twice, so only a plain one).
                if let Expr::Index(inner, j) = &e {
                    if let Expr::Method(recv, m, a, l) = inner.as_ref() {
                        let pure = |x: &Expr| matches!(x, Expr::Ident(_) | Expr::Str(_) | Expr::Int(_)) || matches!(x, Expr::Tuple(xs) if xs.iter().all(|y| matches!(y, Expr::Ident(_) | Expr::Str(_) | Expr::Int(_))));
                        if m == "setdefault" && a.len() == 2 && !self.user_fns.contains(m) && self.py && pure(&a[0]) && matches!(recv.as_ref(), Expr::Ident(_) | Expr::Field(..)) {
                            let (recv, k, v, l) = (recv.clone(), a[0].clone(), a[1].clone(), *l);
                            let cond = Expr::Unary("!", Box::new(Expr::Method(recv.clone(), "has".into(), vec![k.clone()], l)));
                            let set = Stmt::Assign { target: Expr::Index(recv.clone(), Box::new(k.clone())), op: "=", value: v, line };
                            self.pending.push(Stmt::Assign { target: Expr::Index(Box::new(Expr::Index(recv, Box::new(k))), j.clone()), op, value, line });
                            return Ok(Stmt::Expr { expr: Expr::If(Box::new(cond), Box::new(Expr::Block(vec![set], None)), None), line });
                        }
                    }
                }
                // `x = m.setdefault(k, v)`: set m[k] when missing, then x is m[k] (shared, as in Python).
                if let (Expr::Ident(_), "=", Expr::Method(recv, m, a, l)) = (&e, op, &value) {
                    if m == "setdefault" && a.len() == 2 && !self.user_fns.contains(m) && self.py {
                        let (recv, k, v, l) = (recv.clone(), a[0].clone(), a[1].clone(), *l);
                        let cond = Expr::Unary("!", Box::new(Expr::Method(recv.clone(), "has".into(), vec![k.clone()], l)));
                        let set = Stmt::Assign { target: Expr::Index(recv.clone(), Box::new(k.clone())), op: "=", value: v, line };
                        self.pending.push(Stmt::Assign { target: e, op, value: Expr::Index(recv, Box::new(k)), line });
                        return Ok(Stmt::Expr { expr: Expr::If(Box::new(cond), Box::new(Expr::Block(vec![set], None)), None), line });
                    }
                }
                if let (true, Expr::Ident(t), "=", Expr::Call(f, a, _)) = (self.py, &e, op, &value) {
                    if f == "ax_type_of" && a.len() == 1 {
                        self.type_names.insert(t.clone(), value.clone());
                        return Ok(Stmt::Expr { expr: Expr::Block(vec![], None), line });
                    }
                }
                return Ok(Stmt::Assign { target: e, op, value, line });
            }
        }
        self.expr_stmt(e, line)
    }

    // `type(x)` (or a name set to it): x.
    fn type_of(&self, e: &Expr) -> Option<Expr> {
        match self.type_named(e) {
            Expr::Call(f, a, _) if f == "ax_type_of" && a.len() == 1 => Some(a[0].1.clone()),
            _ => None,
        }
    }
    fn type_named(&self, e: &Expr) -> Expr {
        match e {
            Expr::Ident(t) => self.type_names.get(t).cloned().unwrap_or_else(|| e.clone()),
            e => e.clone(),
        }
    }

    // An expression used as a statement, with Python's in-place methods spelled out.
    fn expr_stmt(&mut self, e: Expr, line: usize) -> R<Stmt> {
        // `q.update(type='choice', letter=x)` (or `q.update({'type': 'choice'})`) as a statement on a name:
        // one `q['type'] = ...` per key, so dicts used as records see every key.
        if self.py {
            if let Expr::Method(recv, m, a, _) = &e {
                if let (Expr::Ident(_), "update", [Expr::Map(ps)]) = (recv.as_ref(), m.as_str(), a.as_slice()) {
                    if !ps.is_empty() && ps.iter().all(|(k, _)| matches!(k, Expr::Str(p) if matches!(p.as_slice(), [StrPart::Lit(_)]))) {
                        let sets = ps.iter().map(|(k, v)| Stmt::Assign { target: Expr::Index(recv.clone(), Box::new(k.clone())), op: "=", value: v.clone(), line }).collect();
                        return Ok(Stmt::Expr { expr: Expr::Block(sets, None), line });
                    }
                }
            }
        }
        // `(gone if old else keep).append(l)`: the call on the list the condition picks (not on a copy).
        if let Expr::Method(recv, m, a, l) = &e {
            if let Expr::If(c, t, Some(f)) = recv.as_ref() {
                let bare = |x: &Expr| match x {
                    Expr::Block(st, Some(v)) if st.is_empty() => v.as_ref().clone(),
                    x => x.clone(),
                };
                let (t, f) = (bare(t), bare(f));
                let place = |x: &Expr| matches!(x, Expr::Ident(_) | Expr::Field(..) | Expr::Index(..));
                if place(&t) && place(&f) {
                    let st = self.expr_stmt(Expr::Method(Box::new(t), m.clone(), a.clone(), *l), line)?;
                    let sf = self.expr_stmt(Expr::Method(Box::new(f), m.clone(), a.clone(), *l), line)?;
                    return Ok(Stmt::Expr { expr: Expr::If(c.clone(), Box::new(Expr::Block(vec![st], None)), Some(Box::new(Expr::Block(vec![sf], None)))), line });
                }
            }
        }
        // Python's `xs.extend(ys)` is `xs += ys`; `s.add(x)` on a set() list adds x unless it is there.
        match e {
            // (Not when the program has its own method by that name: a class's `def add(self, x)`.)
            Expr::Method(recv, m, mut a, _) if m == "extend" && a.len() == 1 && !self.own_method(&recv) => Ok(Stmt::Assign { target: *recv, op: "+=", value: a.remove(0), line }),
            Expr::Method(recv, m, mut a, l) if m == "add" && a.len() == 1 && !self.own_method(&recv) => {
                let x = a.remove(0);
                if let Expr::Method(inner, sd, _, _) = recv.as_ref() {
                    if sd == "__setdefault_add" {
                        let Expr::Index(b, k) = inner.as_ref() else { unreachable!() };
                        let has = Expr::Unary("!", Box::new(Expr::Method(b.clone(), "has".into(), vec![(**k).clone()], l)));
                        let put = Stmt::Assign { target: (**inner).clone(), op: "=", value: Expr::Call("__zero".into(), vec![], line), line };
                        let first = Stmt::Expr { expr: Expr::If(Box::new(has), Box::new(Expr::Block(vec![put], None)), None), line };
                        let recv = inner.clone();
                        let cond = Expr::Unary("!", Box::new(Expr::Method(recv.clone(), "contains".into(), vec![x.clone()], l)));
                        let push = Expr::Method(recv, "push".into(), vec![x], l);
                        let then = Stmt::Expr { expr: Expr::If(Box::new(cond), Box::new(Expr::Block(vec![Stmt::Expr { expr: push, line }], None)), None), line };
                        self.pending.push(then);
                        return Ok(first);
                    }
                }
                let cond = Expr::Unary("!", Box::new(Expr::Method(recv.clone(), "contains".into(), vec![x.clone()], l)));
                let push = Expr::Method(recv, "push".into(), vec![x], l);
                Ok(Stmt::Expr { expr: Expr::If(Box::new(cond), Box::new(Expr::Block(vec![Stmt::Expr { expr: push, line }], None)), None), line })
            }
            // `m.setdefault(k, 0)` / `m.setdefault(k, set())` alone on a line (read as `m[k]`) puts the key in:
            // a missing key reads as the empty value, which is set back.
            Expr::Index(recv, k) if self.py && matches!(recv.as_ref(), Expr::Ident(_) | Expr::Field(..) | Expr::Index(..)) => {
                let cond = Expr::Unary("!", Box::new(Expr::Method(recv.clone(), "has".into(), vec![(*k).clone()], line)));
                let set = Stmt::Assign { target: Expr::Index(recv, k), op: "=", value: Expr::Call("__zero".into(), vec![], line), line };
                Ok(Stmt::Expr { expr: Expr::If(Box::new(cond), Box::new(Expr::Block(vec![set], None)), None), line })
            }
            // `m.setdefault(k, v)` as a statement sets m[k] to v when k is missing.
            Expr::Method(recv, m, mut a, l) if m == "setdefault" && a.len() == 2 && !self.user_fns.contains(&m) => {
                let v = a.remove(1);
                let k = a.remove(0);
                let cond = Expr::Unary("!", Box::new(Expr::Method(recv.clone(), "has".into(), vec![k.clone()], l)));
                let set = Stmt::Assign { target: Expr::Index(recv, Box::new(k)), op: "=", value: v, line };
                Ok(Stmt::Expr { expr: Expr::If(Box::new(cond), Box::new(Expr::Block(vec![set], None)), None), line })
            }
            // `s.discard(x)` takes x out of a set() list when it is there.
            // Python's `xs.remove(x)` on a list takes out the first item equal to x (Brevi's remove takes an index).
            Expr::Method(recv, m, mut a, l) if (m == "discard" && !self.user_fns.contains(&m) || m == "remove" && self.py && !self.own_method(&recv)) && a.len() == 1 => {
                let x = a.remove(0);
                let cond = Expr::Method(recv.clone(), "contains".into(), vec![x.clone()], l);
                let at = Expr::Method(recv.clone(), "index_of".into(), vec![x], l);
                let rm = Expr::Method(recv, "remove".into(), vec![at], l);
                Ok(Stmt::Expr { expr: Expr::If(Box::new(cond), Box::new(Expr::Block(vec![Stmt::Expr { expr: rm, line }], None)), None), line })
            }
            e => Ok(Stmt::Expr { expr: e, line }),
        }
    }

    // After a name: is this `, b, c =`?
    fn set_made(e: &Expr) -> bool {
        matches!(e, Expr::Method(_, m, a, _) if m == "unique" && a.is_empty())
    }

    fn is_set(&self, e: &Expr) -> bool {
        Self::set_made(e) || matches!(e, Expr::Ident(n) if self.set_names.contains(n))
    }

    // Is this a value of one of the program's classes (`self`, or a name set from `C(...)`)?
    // `re.sub(p, repl, s)` with repl a def of the program: called on each match, as a lambda would be.
    fn repl_fn(&self, f: &mut Expr, line: usize) {
        if let Expr::Ident(n) = f {
            if self.py && self.user_fns.contains(n) {
                let call = Expr::Call(n.clone(), vec![(None, Expr::Ident("ax_m".into()))], line);
                *f = Expr::Lambda(vec!["ax_m".into()], Box::new(call));
            }
        }
    }
    // A Python program's matches also know their positions (AxMatch); a Brevi program's are plain lists.
    fn re_name(&self, n: &'static str) -> &'static str {
        match (self.py, n) {
            (true, "re_match") => "re_m_match",
            (true, "re_full") => "re_m_full",
            (true, "re_find") => "re_m_find",
            (true, "re_all") => "re_m_all",
            _ => n,
        }
    }
    fn own_method(&self, recv: &Expr) -> bool {
        matches!(recv, Expr::Ident(n) if n == "self_" || self.held.contains(n))
    }

    // Are the tokens from the cursor the rest of `a, b = ...` or `(a, b), c = ...` (names, commas, brackets)?
    fn bare_targets(&self) -> bool {
        let (mut k, mut depth, mut commas) = (0, 0, 0);
        loop {
            match self.peek_at(k) {
                Tok::Ident(_) => {}
                Tok::Sym(",") if depth == 0 => commas += 1,
                Tok::Sym(",") => {}
                Tok::Sym("(") => depth += 1,
                Tok::Sym(")") if depth > 0 => depth -= 1,
                Tok::Sym("=") if depth == 0 => return commas > 0,
                _ => return false,
            }
            k += 1;
        }
    }

    // `(a, (b, c)) = v`: each inner tuple becomes a temporary name, unpacked by a statement after this one.
    fn flat_target(&mut self, e: Expr, line: usize) -> Expr {
        match e {
            Expr::Tuple(xs) => Expr::Tuple(
                xs.into_iter()
                    .map(|x| match x {
                        Expr::Tuple(_) => {
                            let tmp = format!("ax_tu{}_{}", self.pos, self.pending.len());
                            let at = self.pending.len();
                            let inner = self.flat_target(x, line);
                            self.pending.insert(at, Stmt::Assign { target: inner, op: "=", value: Expr::Ident(tmp.clone()), line });
                            Expr::Ident(tmp)
                        }
                        x => x,
                    })
                    .collect(),
            ),
            e => e,
        }
    }

    // A comprehension's loop name, or `(r, d)` inside one: a temporary name unpacked by statements put in `pre`.
    fn comp_target(&mut self, pre: &mut Vec<Stmt>) -> R<String> {
        if !self.eat_sym("(") {
            return self.ident();
        }
        let line = self.line();
        let at = pre.len();
        let mut names = vec![self.comp_target(pre)?];
        while self.eat_sym(",") {
            if self.is_sym(")") {
                break;
            }
            names.push(self.comp_target(pre)?);
        }
        self.expect_sym(")")?;
        let tmp = format!("ax_cu{}", self.pos);
        pre.insert(at, Stmt::Assign { target: Expr::Tuple(names.into_iter().map(Expr::Ident).collect()), op: "=", value: Expr::Ident(tmp.clone()), line });
        Ok(tmp)
    }

    fn pat(&mut self) -> R<Pat> {
        match self.next() {
            Tok::Ident(s) if s == "_" => Ok(Pat::Wild),
            Tok::Ident(s) => {
                if s.chars().next().unwrap().is_uppercase() {
                    let mut args = Vec::new();
                    if self.eat_sym("(") {
                        while !self.is_sym(")") {
                            args.push(self.pat()?);
                            if !self.eat_sym(",") {
                                break;
                            }
                        }
                        self.expect_sym(")")?;
                    }
                    Ok(Pat::Variant(s, args))
                } else {
                    Ok(Pat::Bind(s))
                }
            }
            Tok::Int(n) => Ok(Pat::Int(n)),
            Tok::Sym("-") => match self.next() {
                Tok::Int(n) => Ok(Pat::Int(-n)),
                _ => Err(self.err("expected a number after '-'")),
            },
            Tok::Str(s) => Ok(Pat::Str(s.replace("{{", "{").replace("}}", "}"))),
            Tok::Kw("true") => Ok(Pat::Bool(true)),
            Tok::Kw("false") => Ok(Pat::Bool(false)),
            Tok::Sym("(") => {
                let mut ps = Vec::new();
                while !self.is_sym(")") {
                    ps.push(self.pat()?);
                    if !self.eat_sym(",") {
                        break;
                    }
                }
                self.expect_sym(")")?;
                Ok(Pat::Tuple(ps))
            }
            t => {
                self.pos -= 1;
                Err(self.err(&format!("expected a pattern, found {}", tok_text(&t))))
            }
        }
    }

    pub fn expr(&mut self) -> R<Expr> {
        let e = self.binary(0)?;
        // Python's `a if c else b`; without the `else`, the `if` belongs to a comprehension.
        if self.is_kw("if") {
            let save = self.pos;
            self.next();
            if let Ok(c) = self.binary(0) {
                if self.eat_kw("else") {
                    let other = self.expr()?;
                    return Ok(Expr::If(Box::new(c), Box::new(Expr::Block(vec![], Some(Box::new(e)))), Some(Box::new(Expr::Block(vec![], Some(Box::new(other)))))));
                }
            }
            self.pos = save;
        }
        Ok(e)
    }

    fn binary(&mut self, level: usize) -> R<Expr> {
        const LEVELS: &[&[&str]] = &[
            &["or"],
            &["and"],
            &["==", "!=", "<", ">", "<=", ">=", "~=", "in"],
            &[".."],
            &["|"],
            &["^"],
            &["&"],
            &["<<", ">>"],
            &["+", "-"],
            &["*", "/", "%", "//", "@"],
        ];
        if level == LEVELS.len() {
            return self.unary();
        }
        // Python's `not` binds looser than a comparison: `not 1 <= m <= 12` is `not (1 <= m <= 12)`.
        if level == 2 && self.eat_kw("not") {
            return Ok(Expr::Unary("!", Box::new(self.binary(2)?)));
        }
        let mut lhs = self.binary(level + 1)?;
        // Python's `0 <= x < n` is `0 <= x and x < n`.
        let mut chain: Option<Expr> = None;
        loop {
            // `x not in xs` is `!(x in xs)`.
            let negate = level == 2 && self.peek() == &Tok::Kw("not") && self.peek_at(1) == &Tok::Kw("in");
            if negate {
                self.next();
            }
            let op = match self.peek() {
                Tok::Sym(s) if LEVELS[level].contains(s) => *s,
                Tok::Kw(k) if LEVELS[level].contains(k) => *k,
                _ => break,
            };
            self.next();
            self.skip_newlines();
            let rhs = self.binary(level + 1)?;
            const ORDER: &[&str] = &["<", ">", "<=", ">=", "==", "!="];
            if level == 2 && !negate && ORDER.contains(&op) {
                if let Some(prev) = chain.take() {
                    chain = Some(rhs.clone());
                    lhs = Expr::Binary("and", Box::new(lhs), Box::new(Expr::Binary(op, Box::new(prev), Box::new(rhs))));
                    continue;
                }
                if matches!(rhs, Expr::Ident(_) | Expr::Int(_) | Expr::Float(_) | Expr::Str(_) | Expr::Field(..) | Expr::Call(..) | Expr::Method(..) | Expr::Index(..) | Expr::Binary(..) | Expr::Unary(..)) {
                    chain = Some(rhs.clone());
                }
            } else {
                chain = None;
            }
            // pybridge: an operator on a Python library value is Python's.
            if let Some(x) = crate::pybridge::binop(op, negate, &lhs, &rhs, self.line()) {
                lhs = x;
                continue;
            }
            let empty = |e: &Expr| matches!(e, Expr::List(xs) if xs.is_empty()) || matches!(e, Expr::Map(m) if m.is_empty());
            lhs = if op == ".." {
                Expr::Range(Box::new(lhs), Box::new(rhs))
            } else if (op == "==" || op == "!=") && (empty(&lhs) || empty(&rhs)) {
                // `xs == []` and `m != {}` test for empty.
                let x = if empty(&rhs) { lhs } else { rhs };
                let t = Expr::Method(Box::new(x), "is_empty".into(), vec![], self.line());
                if op == "==" { t } else { Expr::Unary("!", Box::new(t)) }
            } else if let (true, Some(x), Expr::Tuple(ts) | Expr::List(ts)) = (op == "in" || op == "not in", self.type_of(&lhs), &rhs) {
                // `type(c) in (list, str)`: c's kind is exactly one of them.
                let names: Option<Vec<String>> = ts.iter().map(|t| type_test(&Expr::Call("ax_type_of".into(), vec![(None, Expr::Int(0))], 0), t).map(|(_, n)| format!("={n}"))).collect();
                match names {
                    Some(names) => {
                        let t = Expr::Method(Box::new(x), "isinstance".into(), vec![Expr::Str(vec![StrPart::Lit(names.join(","))])], self.line());
                        if op == "in" { t } else { Expr::Unary("!", Box::new(t)) }
                    }
                    None => Expr::Binary(op, Box::new(lhs), Box::new(rhs)),
                }
            } else if let (true, Some(a), Some(b)) = (op == "==" || op == "!=", self.type_of(&lhs), self.type_of(&rhs)) {
                // `type(a) is type(b)`: the same kind.
                Expr::Binary(op, Box::new(Expr::Method(Box::new(a), "type_kind".into(), vec![], self.line())), Box::new(Expr::Method(Box::new(b), "type_kind".into(), vec![], self.line())))
            } else if let (true, Some((x, t))) = (op == "==" || op == "!=", type_test(&self.type_named(&lhs), &rhs).or_else(|| type_test(&self.type_named(&rhs), &lhs))) {
                // `type(v) is int`: v's kind is exactly int (a bool is not).
                let t = Expr::Method(Box::new(x), "isinstance".into(), vec![Expr::Str(vec![StrPart::Lit(format!("={t}"))])], self.line());
                if op == "==" { t } else { Expr::Unary("!", Box::new(t)) }
            } else if (op == "==" || op == "!=") && [&lhs, &rhs].iter().any(|e| matches!(e, Expr::Call(n, a, _) if n == "none" && a.is_empty())) {
                // `x is None` / `x == None`.
                let x = if matches!(&rhs, Expr::Call(n, _, _) if n == "none") { lhs } else { rhs };
                let t = Expr::Method(Box::new(x), "is_none".into(), vec![], self.line());
                if op == "==" { t } else { Expr::Unary("!", Box::new(t)) }
            } else if let (true, Expr::Str(parts)) = (op == "%", &lhs) {
                // Python's `"%d of %s" % (a, b)` fills the string.
                // A tuple held in a name (`'%d: %s' % e`) gives one value per `%` mark.
                let marks = parts.iter().map(|p| if let StrPart::Lit(s) = p { s.replace("%%", "").matches('%').count() } else { 0 }).sum::<usize>();
                let args = match rhs.clone() {
                    Expr::Tuple(xs) => xs,
                    x @ (Expr::Ident(_) | Expr::Field(..) | Expr::Index(..)) if marks >= 2 && !parts.iter().any(|p| matches!(p, StrPart::Lit(s) if s.contains("%("))) => (0..marks).map(|i| Expr::TupleField(Box::new(x.clone()), i)).collect(),
                    x => vec![x],
                };
                match fill_format(parts, args, true, self.line()) {
                    Some(p) => Expr::Str(p),
                    None => Expr::Binary(op, Box::new(lhs), Box::new(rhs)),
                }
            } else if self.py && op == "*" && [&lhs, &rhs].iter().any(|x| matches!(x, Expr::List(xs) if xs.iter().any(|i| match i { Expr::List(_) | Expr::Map(_) => true, Expr::Method(_, m, _, _) => !matches!(m.as_str(), "dec" | "frac" | "frac_div" | "float" | "int"), _ => false }))) {
                // Python's `[[0] * 3] * 3` repeats one inner list, so all rows are the same list.
                return Err(self.err("[[...]] * n makes every row the same list in Python, so changing one row changes them all").fix_hint("write [[0] * 3 for _ in range(3)] for separate rows"));
            } else if op == "*" && matches!(lhs, Expr::List(_)) {
                // Python's `[0] * n` repeats the list.
                Expr::Method(Box::new(lhs), "repeat".into(), vec![rhs], self.line())
            } else if op == "*" && matches!(rhs, Expr::List(_)) {
                Expr::Method(Box::new(rhs), "repeat".into(), vec![lhs], self.line())
            } else if self.py && matches!(op, "<=" | "<" | ">=" | ">") && (self.is_set(&lhs) || self.is_set(&rhs)) {
                // Python's `a <= b` on sets: a is a subset of b.
                let (small, big) = if matches!(op, "<=" | "<") { (lhs, rhs) } else { (rhs, lhs) };
                Expr::Call("ax_subset".into(), vec![(None, small), (None, big), (None, Expr::Bool(matches!(op, "<" | ">")))], self.line())
            } else if op == "/" && self.py {
                // Python's `7 / 2` is 3.5.
                Expr::Call("__truediv".into(), vec![(None, lhs), (None, rhs)], self.line())
            } else {
                Expr::Binary(op, Box::new(lhs), Box::new(rhs))
            };
            if negate {
                lhs = Expr::Unary("!", Box::new(lhs));
            }
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> R<Expr> {
        if self.eat_sym("-") {
            return Ok(Expr::Unary("-", Box::new(self.unary()?)));
        }
        // Python's `~x` on an int is -x - 1.
        if self.py && self.eat_sym("~") {
            let x = self.unary()?;
            return Ok(Expr::Binary("-", Box::new(Expr::Unary("-", Box::new(x))), Box::new(Expr::Int(1))));
        }
        // `not` is accepted as a spelling of `!`.
        if self.eat_sym("!") || self.eat_kw("not") {
            return Ok(Expr::Unary("!", Box::new(self.unary()?)));
        }
        let base = self.postfix()?;
        // Python's `a ** b`: binds tighter than a minus on its left, and groups to the right.
        if self.eat_sym("**") {
            let e = self.unary()?;
            // `10 ** -2` is 0.01 in Python: a negative literal power makes the result a float.
            let base = match &e {
                Expr::Unary("-", x) if self.py && matches!(x.as_ref(), Expr::Int(_)) => Expr::Method(Box::new(base), "float".into(), vec![], self.line()),
                _ => base,
            };
            return Ok(Expr::Binary("**", Box::new(base), Box::new(e)));
        }
        Ok(base)
    }

    fn postfix(&mut self) -> R<Expr> {
        let mut e = self.primary()?;
        loop {
            // A line starting with '.' continues a method chain.
            if self.peek() == &Tok::Newline {
                let mut k = 0;
                while self.peek_at(k) == &Tok::Newline {
                    k += 1;
                }
                if self.peek_at(k) == &Tok::Sym(".") {
                    self.skip_newlines();
                } else {
                    break;
                }
            }
            let line = self.line();
            // pybridge: `.name`, `.name(...)`, `[...]` and `(...)` on a Python library value.
            if let (Tok::Sym("."), Tok::Ident(a)) = (self.peek().clone(), self.peek_at(1).clone()) {
                if let Some(m) = crate::pybridge::native_attr(&e, &a, line) {
                    e = m;
                }
            }
            if crate::pybridge::is_py(&e) && (self.is_sym(".") || self.is_sym("[") || self.is_sym("(")) {
                if self.eat_sym(".") {
                    let name = match self.next() {
                        Tok::Ident(n) => crate::pybridge::unkw(&n),
                        Tok::Kw(k) => k.to_string(),
                        _ => return Err(self.err("expected a name after '.'")),
                    };
                    let args = if self.eat_sym("(") { Some(self.args()?) } else { None };
                    e = crate::pybridge::member(e, &name, args, line);
                } else if self.eat_sym("(") {
                    let args = self.args()?;
                    e = crate::pybridge::invoke(e, args, line);
                } else {
                    self.next();
                    let mut keys = vec![];
                    loop {
                        let a = if self.is_sym(":") { None } else { Some(self.expr()?) };
                        if self.eat_sym(":") {
                            let b = if self.is_sym(":") || self.is_sym("]") || self.is_sym(",") { None } else { Some(self.expr()?) };
                            let c = if self.eat_sym(":") && !self.is_sym("]") && !self.is_sym(",") { Some(self.expr()?) } else { None };
                            keys.push(crate::pybridge::slice(a, b, c, line));
                        } else if let Some(a) = a {
                            keys.push(a);
                        }
                        if !self.eat_sym(",") || self.is_sym("]") {
                            break;
                        }
                    }
                    self.expect_sym("]")?;
                    e = crate::pybridge::index(e, keys, line);
                }
                continue;
            }
            if self.eat_sym(".") {
                match self.next() {
                    Tok::Int(n) => e = Expr::TupleField(Box::new(e), n as usize),
                    t @ (Tok::Ident(_) | Tok::Kw("or" | "match" | "in" | "not" | "and" | "type" | "test")) => {
                        let name = match t {
                            Tok::Ident(n) => n,
                            // `q.type == "choice"`: the field `self.type` was named `type_` (see the lexer).
                            Tok::Kw(k @ ("type" | "match" | "test")) if !self.is_sym("(") => format!("{k}_"),
                            Tok::Kw(k) => k.to_string(),
                            _ => unreachable!(),
                        };
                        if self.eat_sym("(") {
                            let raw = self.args()?;
                            // tkinter: `tk.Label(...)`, `entry.get()` (see gui.rs).
                            if let Some(x) = crate::gui::method(&e, &name, &raw, line) {
                                e = x;
                                continue;
                            }
                            // Python's d.quantize(Decimal("0.01"), rounding=ROUND_HALF_UP): half to even unless ROUND_HALF_UP.
                            // OrderedDict's popitem(last=False) takes the oldest entry; popitem() the newest.
                            // `d.update(k=v, ...)`: the same as `d.update({"k": v, ...})`.
                            if name == "update" && !raw.is_empty() && raw.iter().all(|(k, _)| k.as_deref().is_some_and(|k| k != "__star")) {
                                let ps = raw.iter().map(|(k, v)| (Expr::Str(vec![StrPart::Lit(k.clone().unwrap())]), v.clone())).collect();
                                e = Expr::Method(Box::new(e), name, vec![Expr::Map(ps)], line);
                                continue;
                            }
                            if name == "popitem" && raw.len() <= 1 {
                                let last = !raw.first().is_some_and(|a| matches!(&a.1, Expr::Bool(false)));
                                e = Expr::Method(Box::new(e), "pop_item".into(), vec![Expr::Bool(last)], line);
                                continue;
                            }
                            if name == "quantize" && !raw.is_empty() {
                                let up = raw.iter().any(|a| a.0.as_deref() == Some("rounding") && matches!(&a.1, Expr::Ident(r) if r == "ROUND_HALF_UP"))
                                    || raw.get(1).is_some_and(|a| a.0.is_none() && matches!(&a.1, Expr::Ident(r) if r == "ROUND_HALF_UP"));
                                e = Expr::Method(Box::new(e), name, vec![raw[0].1.clone(), Expr::Bool(up)], line);
                                continue;
                            }
                            if matches!(name.as_str(), "sort" | "sort_by" | "sorted") {
                                if let Some(s) = sort_kw(e.clone(), &raw, line) {
                                    e = s;
                                    continue;
                                }
                            }
                            // json.dumps(x, indent=, separators=, sort_keys=, ensure_ascii=) writes x as Python's json module does.
                            if name == "dumps" && !raw.is_empty() && raw[0].0.is_none() && matches!(&e, Expr::Ident(j) if j == "json") {
                                e = py_dumps(raw, line);
                                continue;
                            }
                            if self.py {
                                // sys.stderr.write(s) and sys.stdout.write(s): the text as is, no line end added.
                                if name == "write" && raw.len() == 1 {
                                    if let Expr::Field(m, f) = &e {
                                        if matches!(m.as_ref(), Expr::Ident(s) if s == "sys") && (f == "stderr" || f == "stdout") {
                                            let s = raw[0].1.clone();
                                            e = if f == "stderr" {
                                                Expr::Call("__eprint".into(), vec![(None, s)], line)
                                            } else {
                                                Expr::Call("print".into(), vec![(None, s), (Some("end".into()), Expr::Str(vec![]))], line)
                                            };
                                            continue;
                                        }
                                    }
                                }
                                let on = |m: &str| matches!(&e, Expr::Ident(x) if x == m);
                                // `sys.stdout.flush()`, `threading.stack_size(n)`, `Exception.__init__(self, msg)`:
                                // nothing to do in native code (output is flushed at exit and before input).
                                let nothing = (name == "flush" && matches!(&e, Expr::Field(m, f) if (f == "stdout" || f == "stderr") && matches!(m.as_ref(), Expr::Ident(s) if s == "sys")))
                                    || (name == "stack_size" && on("threading"))
                                    || (name == "__init__" && matches!(&e, Expr::Ident(x) if x.ends_with("Exception") || x.ends_with("Error") || self.exc_bases.contains_key(x)))
                                    || (name == "__init__" && self.in_exc_class && matches!(&e, Expr::Call(f, _, _) if f == "super" || f == "super_"));
                                if nothing && name == "__init__" && self.in_exc_class {
                                    let given: Vec<&Expr> = raw.iter().map(|a| &a.1).filter(|a| !matches!(a, Expr::Ident(x) if x == "self_")).collect();
                                    self.exc_init_arg = Some(match given.as_slice() {
                                        [Expr::Ident(x)] => Some(x.clone()),
                                        _ => None,
                                    });
                                }
                                if nothing {
                                    e = Expr::Tuple(vec![]);
                                    continue;
                                }
                                // `threading.Thread(target=f, args=(a,))`: run in place when started (`t.start()`,
                                // then `t.join()`), which is what a program that starts one thread and waits sees.
                                if name == "Thread" && on("threading") {
                                    let f = raw.iter().find(|a| a.0.as_deref() == Some("target")).map(|a| a.1.clone());
                                    let fargs = match raw.iter().find(|a| a.0.as_deref() == Some("args")).map(|a| a.1.clone()) {
                                        Some(Expr::Tuple(xs)) | Some(Expr::List(xs)) => xs,
                                        Some(x) => vec![x],
                                        None => vec![],
                                    };
                                    if let Some(Expr::Ident(f)) = f {
                                        e = Expr::Lambda(vec![], Box::new(Expr::Call(f, fargs.into_iter().map(|a| (None, a)).collect(), line)));
                                        continue;
                                    }
                                }
                                if raw.is_empty() && matches!(name.as_str(), "start" | "join") {
                                    if let Expr::Ident(t) = &e {
                                        if self.threads.contains(t) {
                                            e = if name == "start" { Expr::Call(t.clone(), vec![], line) } else { Expr::Tuple(vec![]) };
                                            continue;
                                        }
                                    }
                                    // `threading.Thread(target=go).start()`
                                    if name == "start" {
                                        if let Expr::Lambda(ps, body) = &e {
                                            if ps.is_empty() {
                                                e = (**body).clone();
                                                continue;
                                            }
                                        }
                                    }
                                }
                            }
                            // csv.writer(sys.stdout, delimiter=..., lineterminator=...): the writer is its delimiter
                            // followed by its line ending (literals only), and w.writerow(row) prints the row.
                            if self.py && name == "writer" && matches!(&e, Expr::Ident(m) if m == "csv") && !raw.is_empty() && raw[0].0.is_none() {
                                let is_out = matches!(&raw[0].1, Expr::Field(m, f) if f == "stdout" && matches!(m.as_ref(), Expr::Ident(s) if s == "sys"));
                                let lit = |x: &Expr| match x {
                                    Expr::Str(p) => match p.as_slice() {
                                        [] => Some(String::new()),
                                        [StrPart::Lit(l)] => Some(l.clone()),
                                        _ => None,
                                    },
                                    _ => None,
                                };
                                let mut delim = Some(",".to_string());
                                let mut term = Some("\r\n".to_string());
                                let mut ok = is_out;
                                for (k, v) in &raw[1..] {
                                    match k.as_deref() {
                                        Some("delimiter") => delim = lit(v).filter(|d| d.chars().count() == 1),
                                        Some("lineterminator") => term = lit(v),
                                        _ => ok = false,
                                    }
                                }
                                if let (true, Some(d), Some(t)) = (ok, delim, term) {
                                    e = Expr::Str(vec![StrPart::Lit(format!("{d}{t}"))]);
                                    continue;
                                }
                            }
                            if self.py && matches!(name.as_str(), "writerow" | "writerows") && raw.len() == 1 && raw[0].0.is_none() && !self.user_fns.contains(&name) && !self.own_method(&e) {
                                let row = raw.into_iter().next().unwrap().1;
                                e = Expr::Method(Box::new(e), format!("csv_{name}"), vec![row], line);
                                continue;
                            }
                            let kws_given = raw.iter().any(|(k, _)| k.is_some());
                            let mut args: Vec<Expr> = raw.into_iter().map(|(_, a)| a).collect();
                            // `m.get(k)` is None when k is missing.
                            if name == "get" && args.len() == 1 && !self.own_method(&e) {
                                args.push(Expr::Call("none".into(), vec![], line));
                            }
                            let name = if matches!(&e, Expr::Ident(m) if m == "re") { name } else { python_method(&name, args.len()).unwrap_or(name) };
                            // s.splitlines(True): the flag is in the name now.
                            if name == "lines_keep" {
                                args.clear();
                            }
                            // xs.count(x) in Python counts the items equal to x (s.count(sub) counts a substring).
                            let name = if self.py && name == "count" && args.len() == 1 && !matches!(args[0], Expr::Lambda(..)) && !self.own_method(&e) { "count_of".to_string() } else { name };
                            // Python's `"{} of {:>5}".format(a, b)` fills the string.
                            if name == "format" {
                                if let Expr::Str(parts) = &e {
                                    if let Some(p) = fill_format(parts, args.clone(), false, line) {
                                        e = Expr::Str(p);
                                        continue;
                                    }
                                }
                            }
                            // Flags at the end of a re call (`re.fullmatch(p, s, re.ASCII)`, `re.compile(p, re.I)`):
                            // IGNORECASE goes into the pattern as (?i); ASCII is dropped. Other flags stay (not supported).
                            if matches!(&e, Expr::Ident(m) if m == "re") && args.len() >= 2 {
                                let mut inline = String::new();
                                let mut ok = true;
                                let mut flag_words: Vec<String> = vec![];
                                fn words(x: &Expr, out: &mut Vec<String>) -> bool {
                                    match x {
                                        Expr::Field(b, f) if matches!(b.as_ref(), Expr::Ident(r) if r == "re") => {
                                            out.push(f.clone());
                                            true
                                        }
                                        Expr::Binary("|", a, b) => words(a, out) && words(b, out),
                                        _ => false,
                                    }
                                }
                                if words(args.last().unwrap(), &mut flag_words) {
                                    for w in &flag_words {
                                        match w.as_str() {
                                            "I" | "IGNORECASE" => inline.push('i'),
                                            "S" | "DOTALL" => inline.push('s'),
                                            "M" | "MULTILINE" => inline.push('m'),
                                            "A" | "ASCII" => {}
                                            _ => ok = false,
                                        }
                                    }
                                    if ok {
                                        args.pop();
                                        if !inline.is_empty() {
                                            // (?i) first, on its own: the prelude reads it before (?s) and (?m).
                                            let rest: String = inline.chars().filter(|c| *c != 'i').collect();
                                            let mut flag = if inline.contains('i') { "(?i)".to_string() } else { String::new() };
                                            if !rest.is_empty() {
                                                flag.push_str(&format!("(?{rest})"));
                                            }
                                            match &mut args[0] {
                                                Expr::Str(parts) => match parts.first_mut() {
                                                    Some(StrPart::Lit(l)) => l.insert_str(0, &flag),
                                                    _ => parts.insert(0, StrPart::Lit(flag)),
                                                },
                                                a0 => {
                                                    let p0 = std::mem::replace(a0, Expr::Int(0));
                                                    *a0 = Expr::Binary("+", Box::new(Expr::Str(vec![StrPart::Lit(flag)])), Box::new(p0));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // Python's re module: a compiled pattern is its string, a match is read as [whole, group1, ...] (empty for none).
                            if matches!(&e, Expr::Ident(m) if m == "re") {
                                let r = |s: Expr, n: &str, a: Vec<Expr>| Expr::Method(Box::new(s), n.into(), a, line);
                                let done = match (name.as_str(), args.len()) {
                                    ("compile", 1) => Some(args.remove(0)),
                                    ("match", 2) => Some(r(args.remove(1), self.re_name("re_match"), vec![args.remove(0)])),
                                    ("fullmatch", 2) => Some(r(args.remove(1), self.re_name("re_full"), vec![args.remove(0)])),
                                    ("search", 2) => Some(r(args.remove(1), self.re_name("re_find"), vec![args.remove(0)])),
                                    ("findall", 2) => Some(r(args.remove(1), "re_findall", vec![args.remove(0)])),
                                    ("finditer", 2) => Some(r(args.remove(1), self.re_name("re_all"), vec![args.remove(0)])),
                                    ("split", 2) => Some(r(args.remove(1), "re_split", vec![args.remove(0)])),
                                    ("sub", 3) => {
                                        let s = args.remove(2);
                                        let mut a: Vec<Expr> = args.drain(..).collect();
                                        self.repl_fn(&mut a[1], line);
                                        Some(r(s, "re_sub", a))
                                    }
                                    _ => None,
                                };
                                if let Some(d) = done {
                                    e = d;
                                    continue;
                                }
                            }
                            // pat.match(s), pat.fullmatch(s), pat.search(s), pat.findall(s), pat.sub(r, s) on a compiled pattern
                            // (a program's own method of that name keeps it).
                            if !self.user_fns.contains(&name) {
                                let r = |s: Expr, n: &str, a: Vec<Expr>| Expr::Method(Box::new(s), n.into(), a, line);
                                let m = match (name.as_str(), args.len()) {
                                    ("match", 1) => Some(self.re_name("re_match")),
                                    ("fullmatch", 1) => Some(self.re_name("re_full")),
                                    ("search", 1) => Some(self.re_name("re_find")),
                                    ("findall", 1) => Some("re_findall"),
                                    ("finditer", 1) => Some(self.re_name("re_all")),
                                    _ => None,
                                };
                                if let Some(m) = m {
                                    e = r(args.remove(0), m, vec![e]);
                                    continue;
                                }
                                if name == "sub" && args.len() == 2 {
                                    let s = args.remove(1);
                                    let mut f = args.remove(0);
                                    self.repl_fn(&mut f, line);
                                    e = r(s, "re_sub", vec![e, f]);
                                    continue;
                                }
                            }
                            // m.group(n), m.group(), m.groups() on a match.
                            if name == "group" && args.len() <= 1 && !self.user_fns.contains(&name) {
                                let i = if args.is_empty() { Expr::Int(0) } else { args.remove(0) };
                                e = Expr::Index(Box::new(e), Box::new(i));
                                continue;
                            }
                            if name == "groups" && args.is_empty() && !self.user_fns.contains(&name) {
                                let m = if self.py { "re_groups" } else { "skip" };
                                e = Expr::Method(Box::new(e), m.into(), if self.py { vec![] } else { vec![Expr::Int(1)] }, line);
                                continue;
                            }
                            // m.start(), m.end(n), m.span() on a Python match.
                            if self.py && matches!(name.as_str(), "start" | "end" | "span") && args.len() <= 1 && !self.user_fns.contains(&name) && !self.own_method(&e) {
                                let i = if args.is_empty() { Expr::Int(0) } else { args.remove(0) };
                                e = Expr::Method(Box::new(e), format!("re_{name}"), vec![i], line);
                                continue;
                            }
                            // csv.reader(sys.stdin), csv.reader(lines) or csv.reader(text.splitlines()) give the rows.
                            if name == "reader" && args.len() == 1 && !kws_given && matches!(&e, Expr::Ident(m) if m == "csv") {
                                let src = match args.remove(0) {
                                    Expr::Call(f, a, l) if f == "lines" && a.is_empty() => Expr::Call("stdin".into(), vec![], l),
                                    other => other,
                                };
                                e = Expr::Method(Box::new(src), "csv_rows".into(), vec![], line);
                                continue;
                            }
                            // csv.DictReader(src) with the default comma and the first row as the header.
                            if name == "DictReader" && args.len() == 1 && !kws_given && matches!(&e, Expr::Ident(m) if m == "csv") {
                                let src = match args.remove(0) {
                                    Expr::Call(f, a, l) if f == "lines" && a.is_empty() => Expr::Call("stdin".into(), vec![], l),
                                    other => other,
                                };
                                e = Expr::Method(Box::new(src), "csv_dicts".into(), vec![], line);
                                continue;
                            }
                            // date.fromisoformat(s) and datetime.date.fromisoformat(s) are date(s).
                            if name == "fromisoformat" && args.len() == 1 && matches!(&e, Expr::Ident(m) if m == "date") || matches!(&e, Expr::Field(m, f) if f == "date" && matches!(m.as_ref(), Expr::Ident(d) if d == "datetime")) && name == "fromisoformat" {
                                e = Expr::Call("date".into(), vec![(None, args.remove(0))], line);
                                continue;
                            }
                            // datetime.date(y, m, d) is date(y, m, d); datetime.strptime(s, "%Y-%m-%d") (any alias) reads the date.
                            // `datetime.date(*parts)`: one spread argument.
                            if name == "date" && args.len() == 1 && matches!(&e, Expr::Ident(m) if m == "datetime") {
                                let mut a: Vec<(Option<String>, Expr)> = vec![(Some("__star".into()), args.remove(0))];
                                e = python_call("date", &mut a, line).unwrap();
                                continue;
                            }
                            if name == "date" && args.len() == 3 && matches!(&e, Expr::Ident(m) if m == "datetime") {
                                let mut a: Vec<(Option<String>, Expr)> = args.drain(..).map(|x| (None, x)).collect();
                                e = python_call("date", &mut a, line).unwrap();
                                continue;
                            }
                            if name == "strptime" && args.len() == 2 && !self.user_fns.contains(&name) && matches!(&args[1], Expr::Str(p) if matches!(p.as_slice(), [StrPart::Lit(f)] if f == "%Y-%m-%d")) {
                                args.pop();
                                e = Expr::Call("date".into(), vec![(None, args.remove(0))], line);
                                continue;
                            }
                            // .date() on a date read that way is the date itself.
                            if name == "date" && args.is_empty() && matches!(&e, Expr::Call(f, _, _) if f == "date") {
                                continue;
                            }
                            if args.is_empty() && !self.user_fns.contains(&name) {
                                let one = |x: Expr, n: &str| Expr::Method(Box::new(x), n.into(), vec![], line);
                                let done = match name.as_str() {
                                    // d.isoformat() is str(d); d.toordinal() counts days from 0001-01-01 as day 1.
                                    "isoformat" => Some(Expr::Call("str".into(), vec![(None, e.clone())], line)),
                                    "toordinal" => Some(Expr::Binary("+", Box::new(one(e.clone(), "days")), Box::new(Expr::Int(719163)))),
                                    // Python's d.weekday() is 0 for Monday; d.isoweekday() is 1 for Monday.
                                    "weekday" if self.py => Some(Expr::Binary("-", Box::new(one(e.clone(), "weekday")), Box::new(Expr::Int(1)))),
                                    "isoweekday" => Some(one(e.clone(), "weekday")),
                                    _ => None,
                                };
                                if let Some(d) = done {
                                    e = d;
                                    continue;
                                }
                            }
                            // calendar.monthrange(y, m) gives (weekday of the 1st, days in the month); calendar.isleap(y).
                            if matches!(&e, Expr::Ident(m) if m == "calendar") {
                                if name == "monthrange" && args.len() == 2 {
                                    let mo = args.pop().unwrap();
                                    e = Expr::Method(Box::new(args.remove(0)), "month_range".into(), vec![mo], line);
                                    continue;
                                }
                                if name == "isleap" && args.len() == 1 {
                                    e = Expr::Method(Box::new(args.remove(0)), "is_leap".into(), vec![], line);
                                    continue;
                                }
                            }
                            // sys.setrecursionlimit(n): native code has no recursion limit to raise.
                            if name == "setrecursionlimit" && matches!(&e, Expr::Ident(s) if s == "sys") {
                                e = Expr::Tuple(vec![]);
                                continue;
                            }
                            // json.loads(s) and json.load(f) read JSON.
                            if (name == "loads" || name == "load") && args.len() == 1 && matches!(&e, Expr::Ident(j) if j == "json") {
                                let src = match args.remove(0) {
                                    Expr::Field(m, f) if f == "stdin" && matches!(m.as_ref(), Expr::Ident(s) if s == "sys") => Expr::Call("stdin".into(), vec![], line),
                                    // `sys.stdin` already read as its lines.
                                    Expr::Call(f, a, l) if f == "lines" && a.is_empty() => Expr::Call("stdin".into(), vec![], l),
                                    other => other,
                                };
                                e = Expr::Call("json".into(), vec![(None, src)], line);
                                continue;
                            }
                            // sys.exit(n) ends the program with that code; sys.exit("msg") prints msg and exits with 1.
                            if name == "exit" && args.len() <= 1 && matches!(&e, Expr::Ident(s) if s == "sys") {
                                let code = if args.is_empty() { Expr::Int(0) } else { args.remove(0) };
                                e = Expr::Method(Box::new(code), "exit_with".into(), vec![], line);
                                continue;
                            }
                            // sys.stdout.write(s) prints s as it is.
                            if name == "write" && args.len() == 1 && matches!(&e, Expr::Field(m, f) if f == "stdout" && matches!(m.as_ref(), Expr::Ident(s) if s == "sys")) {
                                e = Expr::Call("print".into(), vec![(None, args.remove(0)), (Some("end".into()), Expr::Str(vec![]))], line);
                                continue;
                            }
                            // A deque is a list: popleft() takes the first item, appendleft(x) puts one in front.
                            if name == "popleft" && args.is_empty() {
                                e = Expr::Method(Box::new(e), "pop_at".into(), vec![Expr::Int(0)], line);
                                continue;
                            }
                            if name == "appendleft" && args.len() == 1 {
                                e = Expr::Method(Box::new(e), "insert".into(), vec![Expr::Int(0), args.remove(0)], line);
                                continue;
                            }
                            // heapq.heappush(h, x), heapq.heappop(h), heapq.heapify(h) work on a plain list, as in Python.
                            if matches!(&e, Expr::Ident(m) if m == "heapq") && !args.is_empty() {
                                if let Some(m) = heapq_method(&name, args.len()) {
                                    let h = args.remove(0);
                                    e = Expr::Method(Box::new(h), m.into(), args, line);
                                    continue;
                                }
                            }
                            // math.log(x), math.sin(x), math.hypot(x, y)...: the typed build's float methods (m_log...),
                            // which raise ValueError where Python's do (math.log(0), math.sqrt(-1)).
                            if self.py && matches!(&e, Expr::Ident(m) if m == "math") && !kws_given {
                                if let Some(m) = math_method(&name, args.len()) {
                                    let x = args.remove(0);
                                    e = Expr::Method(Box::new(x), m.into(), args, line);
                                    continue;
                                }
                            }
                            // time.time(), time.perf_counter(), time.monotonic(): seconds as a float.
                            if self.py && matches!(&e, Expr::Ident(m) if m == "time") && args.is_empty() && matches!(name.as_str(), "time" | "perf_counter" | "monotonic") {
                                e = Expr::Method(Box::new(Expr::Float(0.0)), format!("m_{name}"), vec![], line);
                                continue;
                            }
                            // math.isqrt(n): the whole square root of an int.
                            if matches!(&e, Expr::Ident(m) if m == "math") && args.len() == 1 && name == "isqrt" {
                                e = Expr::Method(Box::new(args.remove(0)), "isqrt".into(), vec![], line);
                                continue;
                            }
                            // math.sqrt(x), math.floor(x), math.ceil(x); sys.stdin.read() is stdin().
                            if matches!(&e, Expr::Ident(m) if m == "math") && args.len() == 1 && matches!(name.as_str(), "sqrt" | "floor" | "ceil") {
                                let x = args.remove(0);
                                let f = Expr::Method(Box::new(Expr::Method(Box::new(x), "float".into(), vec![], line)), name.clone(), vec![], line);
                                e = if name == "sqrt" { f } else { Expr::Method(Box::new(f), "int".into(), vec![], line) };
                                continue;
                            }
                            // d.scaleb(k) on a Decimal: d times 10**k.
                            if name == "scaleb" && args.len() == 1 {
                                let k = match &args[0] { Expr::Int(k) => Some(*k), Expr::Unary("-", x) => if let Expr::Int(k) = x.as_ref() { Some(-k) } else { None }, _ => None };
                                if let Some(k) = k.filter(|k| (-18..=18).contains(k)) {
                                    let ten = |n: i64| Expr::Method(Box::new(Expr::Int(10i64.pow(n as u32))), "dec".into(), vec![], line);
                                    e = if k >= 0 { Expr::Binary("*", Box::new(e), Box::new(ten(k))) } else { Expr::Binary("/", Box::new(e), Box::new(ten(-k))) };
                                    continue;
                                }
                            }
                            // sys.stdin.readline(): the next line with its "\n" ("" at the end of input).
                            if matches!(&e, Expr::Call(f, a, _) if f == "lines" && a.is_empty()) && name == "readline" && args.is_empty() {
                                e = Expr::Call("__readline".into(), vec![], line);
                                continue;
                            }
                            // open(p).read() is the file's text; open(p).readlines() its lines, each with its "\n".
                            if matches!(&e, Expr::Call(f, _, _) if f == "__open_r") && args.is_empty() && matches!(name.as_str(), "read" | "readlines") {
                                let Expr::Call(_, a, _) = e else { unreachable!() };
                                e = Expr::Call("read".into(), a, line);
                                if name == "readlines" {
                                    e = Expr::Method(Box::new(e), "lines_keep".into(), vec![], line);
                                }
                                continue;
                            }
                            if matches!(&e, Expr::Call(f, a, _) if f == "lines" && a.is_empty()) && name == "read" && args.is_empty() {
                                e = Expr::Call("stdin".into(), vec![], line);
                                continue;
                            }
                            // `m.setdefault(k, [])` is `m[k]`: a missing key reads as the empty value.
                            // (Only when a method follows, `m.setdefault(k, []).append(x)`: alone or assigned it puts the key in
                            // first, so that a plain dict, whose missing keys raise KeyError, has it.)
                            if name == "setdefault" && args.len() == 2 && empty_literal(&args[1]) && (!self.py || self.is_sym(".")) {
                                e = Expr::Index(Box::new(e), Box::new(args.remove(0)));
                                // `m.setdefault(k, set()).add(x)`: add() first reads m[k] to test x, so the key
                                // must be put in before (a plain dict's missing key raises KeyError).
                                if self.py && matches!(self.peek_at(1), Tok::Ident(a) if a == "add") && !self.user_fns.iter().any(|f| f == "add") {
                                    e = Expr::Method(Box::new(e), "__setdefault_add".into(), vec![], line);
                                }
                                continue;
                            }
                            // `xs.to_list()` / `xs.copy()`: a list already is one, so this is a copy.
                            // (A Python program with its own `def copy(self)` keeps `.copy()` until the value's type is known.)
                            let own_copy = name == "copy" && self.py && self.user_fns.iter().any(|f| f == "copy");
                            let name = if matches!(name.as_str(), "to_list" | "to_vec" | "copy" | "collect") && args.is_empty() && !own_copy {
                                args.push(Expr::Int(0));
                                "skip".to_string()
                            } else {
                                name
                            };
                            // `n.range()` counts 0..n.
                            if name == "range" && args.is_empty() {
                                e = Expr::Range(Box::new(Expr::Int(0)), Box::new(e));
                                continue;
                            }
                            // Python's `", ".join(xs)` is `xs.join(", ")`.
                            e = if name == "join" && args.len() == 1 && matches!(e, Expr::Str(_)) && (self.py || !matches!(args[0], Expr::Str(_))) {
                                Expr::Method(Box::new(args.remove(0)), name, vec![e], line)
                            } else {
                                Expr::Method(Box::new(e), name, args, line)
                            };
                        } else if self.py && matches!(&e, Expr::Ident(m) if m == "operator") && operator_fn(&name, line).is_some() {
                            // `operator.lt` as a value: a two-argument lambda.
                            e = operator_fn(&name, line).unwrap();
                        } else if self.py && matches!(&e, Expr::Ident(m) if m == "math") && matches!(name.as_str(), "pi" | "e" | "tau" | "inf" | "nan") {
                            // math.pi, math.e, math.tau; math.inf and math.nan come from a float method.
                            e = match name.as_str() {
                                "pi" => Expr::Float(std::f64::consts::PI),
                                "e" => Expr::Float(std::f64::consts::E),
                                "tau" => Expr::Float(std::f64::consts::TAU),
                                n => Expr::Method(Box::new(Expr::Float(0.0)), format!("m_{n}"), vec![], self.line()),
                            };
                        } else if name == "argv" && matches!(&e, Expr::Ident(m) if m == "sys") {
                            // sys.argv: the program's name, then its arguments.
                            e = Expr::Binary("+", Box::new(Expr::List(vec![Expr::Str(vec![StrPart::Lit("main".into())])])), Box::new(Expr::Call("args".into(), vec![], line)));
                        } else if name == "stdin" && matches!(&e, Expr::Ident(m) if m == "sys") {
                            e = Expr::Call("lines".into(), vec![], line);
                        } else {
                            e = Expr::Field(Box::new(e), name);
                        }
                    }
                    _ => return Err(self.err("expected a field or method name after '.'")),
                }
            } else if self.eat_sym("[") {
                // `xs[a:b]`, `xs[a:]`, `xs[:b]` slice like `slice(a, b)`.
                let line = self.line();
                let start = if self.is_sym(":") { None } else { Some(self.expr()?) };
                if self.eat_sym(":") {
                    let mut args = vec![start.clone().unwrap_or(Expr::Int(0))];
                    if !self.is_sym("]") && !self.is_sym(":") {
                        args.push(self.expr()?);
                    }
                    // Python's step: `xs[::-1]` reverses, `xs[::2]` takes every second item.
                    let step = if self.eat_sym(":") && !self.is_sym("]") { Some(self.expr()?) } else { None };
                    self.expect_sym("]")?;
                    // Python's xs[a:b:k] with a step other than 1 (and not a bare xs[::-1]): picked as
                    // Python does, counting down from a for a negative step.
                    let one = |k: &Expr| matches!(k, Expr::Int(1));
                    let minus_one = |k: &Expr| matches!(k, Expr::Int(-1)) || matches!(k, Expr::Unary("-", n) if matches!(n.as_ref(), Expr::Int(1)));
                    if self.py && step.as_ref().is_some_and(|k| !one(k) && !(minus_one(k) && start.is_none() && args.len() == 1)) {
                        let nosl = || Expr::Int(i64::MIN + 1);
                        let a = start.clone().unwrap_or_else(nosl);
                        let b = if args.len() > 1 { args.pop().unwrap() } else { nosl() };
                        e = Expr::Method(Box::new(e), "slice_step".into(), vec![a, b, step.unwrap()], line);
                        continue;
                    }
                    if start.is_some() || args.len() > 1 || step.is_none() {
                        e = Expr::Method(Box::new(e), "slice".into(), args, line);
                    }
                    match step {
                        Some(Expr::Unary("-", n)) if matches!(*n, Expr::Int(1)) => e = Expr::Method(Box::new(e), "rev".into(), vec![], line),
                        Some(Expr::Int(-1)) => e = Expr::Method(Box::new(e), "rev".into(), vec![], line),
                        Some(Expr::Int(1)) | None => {}
                        Some(k) => e = Expr::Method(Box::new(e), "step".into(), vec![k], line),
                    }
                } else {
                    self.expect_sym("]")?;
                    let Some(idx) = start else { return Err(self.err("expected an index")) };
                    e = Expr::Index(Box::new(e), Box::new(idx));
                }
            } else if self.is_sym("(") && matches!(&e, Expr::Lambda(ps, _) if !ps.is_empty()) {
                // `(lambda s: (s[0], s[1]))(x)` calls the lambda at once: a block that names its arguments.
                self.eat_sym("(");
                let args: Vec<Expr> = self.args()?.into_iter().map(|(_, a)| a).collect();
                let Expr::Lambda(ps, body) = e else { unreachable!() };
                if args.len() != ps.len() {
                    return Err(self.err("wrong number of arguments for this lambda"));
                }
                let lets = ps.into_iter().zip(args).map(|(p, a)| Stmt::Let { name: Pat::Bind(p), ty: None, value: a, mutable: false, line }).collect();
                e = Expr::Block(lets, Some(body));
            } else if self.is_sym("(") && matches!(e, Expr::Index(..) | Expr::Field(..) | Expr::TupleField(..)) {
                // `ops[1](3, 4)`: call a function value taken from a list, tuple or field.
                self.eat_sym("(");
                let args = self.args()?.into_iter().map(|(_, a)| a).collect();
                e = Expr::Method(Box::new(e), "__call".into(), args, line);
            } else if self.eat_sym("?") {
                e = Expr::Try(Box::new(e));
            } else {
                break;
            }
        }
        Ok(e)
    }

    // Arguments after '(' up to and including ')'. Supports name: value.
    fn args(&mut self) -> R<Vec<(Option<String>, Expr)>> {
        let mut args = Vec::new();
        while !self.is_sym(")") {
            let name = match (self.peek().clone(), self.peek_at(1).clone()) {
                // Python keywords: `name=value` (`key=f`, `reverse=True`).
                (Tok::Ident(n), Tok::Sym(":")) | (Tok::Ident(n), Tok::Sym("=")) => {
                    self.next();
                    self.next();
                    Some(n)
                }
                _ => None,
            };
            // `sum(*row for row in grid)` (Python 3.15) passes the joined lists.
            let flat = self.eat_sym("*");
            let e = self.expr()?;
            // `min(x for x in xs)` passes the list the loop builds.
            let e = if self.eat_kw("for") {
                self.flat_comprehension(e, flat)?
            } else if flat {
                // Python's `print(*xs)`: each item is its own argument.
                args.push((Some("__star".into()), e));
                if !self.eat_sym(",") {
                    break;
                }
                continue;
            } else {
                e
            };
            args.push((name, e));
            if !self.eat_sym(",") {
                break;
            }
        }
        self.expect_sym(")")?;
        Ok(args)
    }

    // The rest of `elem for x in xs if c`, as xs.filter(x => c).map(x => elem).
    fn comprehension(&mut self, elem: Expr) -> R<Expr> {
        let line = self.line();
        let paren = self.eat_sym("(");
        let mut pre = Vec::new();
        let mut names = vec![self.comp_target(&mut pre)?];
        while self.eat_sym(",") {
            if self.is_sym(")") {
                break;
            }
            names.push(self.comp_target(&mut pre)?);
        }
        if paren {
            self.expect_sym(")")?;
            // `for (a, b), c in ...`: the brackets held only the first name.
            if self.is_sym(",") {
                let tmp = format!("ax_cu{}", self.pos);
                pre.insert(0, Stmt::Assign { target: Expr::Tuple(names.into_iter().map(Expr::Ident).collect()), op: "=", value: Expr::Ident(tmp.clone()), line });
                names = vec![tmp];
                while self.eat_sym(",") {
                    names.push(self.comp_target(&mut pre)?);
                }
            }
        }
        let wrap = |x: Expr| if pre.is_empty() { x } else { Expr::Block(pre.clone(), Some(Box::new(x))) };
        if !self.eat_kw("in") {
            return Err(self.err("expected in").fix_hint("write [x * 2 for x in xs]"));
        }
        let mut e = self.expr()?;
        if crate::pybridge::is_py(&e) {
            crate::pybridge::note_for(&Pat::Tuple(names.iter().map(|n| Pat::Bind(n.clone())).collect()), &e);
        } else if crate::pybridge::active() {
            crate::pybridge::note_comp_native(&names);
        }
        // Comprehensions sit inside brackets, where Python lets `for`/`if` start a new line.
        self.skip_newlines();
        // `x for a in A for x in a`: each a gives a list; they are joined.
        if self.eat_kw("for") {
            let inner = wrap(self.comprehension(elem)?);
            return Ok(Expr::Method(Box::new(Expr::Method(Box::new(e), "map".into(), vec![Expr::Lambda(names, Box::new(inner))], line)), "flatten".into(), vec![], line));
        }
        if self.eat_kw("if") {
            // The condition uses Python truth (`if s.trim()`): `not not c` reads it that way.
            let c = self.expr()?;
            self.skip_newlines();
            let c = if matches!(&c, Expr::Binary(op, ..) if ["==", "!=", "<", ">", "<=", ">=", "in", "~=", "and", "or"].contains(op)) || matches!(c, Expr::Bool(_)) { c } else { Expr::Unary("!", Box::new(Expr::Unary("!", Box::new(c)))) };
            e = Expr::Method(Box::new(e), "filter".into(), vec![Expr::Lambda(names.clone(), Box::new(wrap(c)))], line);
        }
        // `[c for c in s]` over a text is a list of its characters, so Python keeps the map there.
        let filtered = matches!(&e, Expr::Method(_, m, _, _) if m == "filter");
        if !matches!(&elem, Expr::Ident(n) if names.len() == 1 && n == &names[0]) || !pre.is_empty() || (self.py && !filtered) {
            e = Expr::Method(Box::new(e), "map".into(), vec![Expr::Lambda(names, Box::new(wrap(elem)))], line);
        }
        Ok(e)
    }

    // `[*xs for xs in xss]` (Python 3.15) joins the lists, or the characters of strings.
    fn flat_comprehension(&mut self, elem: Expr, flat: bool) -> R<Expr> {
        let line = self.line();
        let e = self.comprehension(elem)?;
        Ok(if flat { Expr::Method(Box::new(e), "flatten".into(), vec![], line) } else { e })
    }

    // Is the '(' at the cursor the start of a lambda's parameter list?
    fn paren_is_lambda(&self) -> bool {
        let mut k = 1;
        loop {
            match self.peek_at(k) {
                Tok::Ident(_) | Tok::Sym(",") => k += 1,
                Tok::Sym(")") => return self.peek_at(k + 1) == &Tok::Sym("=>"),
                _ => return false,
            }
        }
    }

    fn primary(&mut self) -> R<Expr> {
        let line = self.line();
        if let (Tok::Ident(name), Tok::Sym("=>")) = (self.peek().clone(), self.peek_at(1).clone()) {
            self.next();
            self.next();
            let body = self.expr()?;
            return Ok(Expr::Lambda(vec![name], Box::new(body)));
        }
        if self.is_sym("(") && self.paren_is_lambda() {
            self.next();
            let mut names = Vec::new();
            while !self.is_sym(")") {
                names.push(self.ident()?);
                self.eat_sym(",");
            }
            self.next();
            self.expect_sym("=>")?;
            let body = self.expr()?;
            return Ok(Expr::Lambda(names, Box::new(body)));
        }
        match self.next() {
            Tok::Int(n) => Ok(Expr::Int(n)),
            Tok::Float(f) => Ok(Expr::Float(f)),
            Tok::Kw("true") => Ok(Expr::Bool(true)),
            Tok::Kw("false") => Ok(Expr::Bool(false)),
            Tok::Str(s) => Ok(Expr::Str(self.interp(&s, line)?)),
            // Python's `lambda x, y: expr`.
            Tok::Ident(name) if name == "lambda" && matches!(self.peek(), Tok::Ident(_) | Tok::Sym(":")) => {
                let mut names = Vec::new();
                // `lambda k=k: ...` keeps the value k has now.
                let mut defaults: Vec<(Option<String>, Expr)> = Vec::new();
                while let Tok::Ident(n) = self.peek().clone() {
                    self.next();
                    if self.eat_sym("=") {
                        let d = self.expr()?;
                        defaults.push((Some(n), d));
                    } else {
                        names.push(n);
                    }
                    if !self.eat_sym(",") {
                        break;
                    }
                }
                self.expect_sym(":")?;
                let body = self.expr()?;
                if defaults.is_empty() {
                    return Ok(Expr::Lambda(names, Box::new(body)));
                }
                if crate::gui::active() {
                    defaults.push((None, Expr::Lambda(names, Box::new(body))));
                    return Ok(Expr::Call("tk:lambda".into(), defaults, line));
                }
                // Elsewhere: the names are bound to their values when the lambda runs.
                let lets = defaults.into_iter().map(|(n, d)| Stmt::Let { name: Pat::Bind(n.unwrap_or_default()), ty: None, value: d, mutable: false, line }).collect();
                Ok(Expr::Lambda(names, Box::new(Expr::Block(lets, Some(Box::new(body))))))
            }
            // Python's `if __name__ == "__main__":` is always true here.
            Tok::Ident(name) if name == "__name__" => Ok(Expr::Str(vec![StrPart::Lit("__main__".into())])),
            // Python's None: an int, float, str or tuple slot that is not set yet.
            Tok::Ident(name) if name == "None" && !self.is_sym("(") => Ok(Expr::Call("none".into(), vec![], line)),
            Tok::Ident(name) => {
                if self.eat_sym("(") {
                    let mut args = self.args()?;
                    if let Some(x) = crate::gui::bare_call(&name, &args, line) {
                        return Ok(x);
                    }
                    if let Some(x) = crate::pybridge::call_hook(&name, &args, &self.user_fns, line) {
                        return Ok(x);
                    }
                    // Python's len(xs), sum(xs), sorted(xs), min(xs), max(xs) are the methods.
                    let m = match name.as_str() {
                        "len" | "sum" | "min" | "max" | "enumerate" | "int" | "float" => Some(name.as_str()),
                        "sorted" => Some("sort"),
                        "reversed" => Some("rev"),
                        "ord" => Some("code"),
                        "bin" => Some("bin_text"),
                        "hex" => Some("hex_text"),
                        "oct" => Some("oct_text"),
                        "chr" => Some("char"),
                        "unique" => Some("unique"),
                        _ => None,
                    };
                    // sum(xs, start): the start plus the sum.
                    if name == "sum" && !self.user_fns.contains(&name) && args.len() == 2 {
                        let start = args.remove(1).1;
                        let xs = args.remove(0).1;
                        return Ok(Expr::Binary("+", Box::new(start), Box::new(Expr::Method(Box::new(xs), "sum".into(), vec![], line))));
                    }
                    // min(a, b, c, ...): min(min(a, b), c), ... (the prelude's min and max take two values).
                    if (name == "min" || name == "max") && !self.user_fns.contains(&name) && args.len() > 2 && args.iter().all(|a| a.0.is_none()) {
                        let mut it = args.into_iter().map(|a| a.1);
                        let first = it.next().unwrap();
                        return Ok(it.fold(first, |acc, x| Expr::Call(name.clone(), vec![(None, acc), (None, x)], line)));
                    }
                    // min(xs, default=d), max(xs, default=d) and with key=f too: d when xs is empty.
                    if (name == "min" || name == "max") && !self.user_fns.contains(&name) && (2..=3).contains(&args.len()) && args[0].0.is_none() && args.last().is_some_and(|a| a.0.as_deref() == Some("default")) {
                        let d = args.pop().unwrap().1;
                        let xs = args.remove(0).1;
                        if let Some((_, f)) = args.pop().filter(|a| a.0.as_deref() == Some("key")) {
                            let mut a = vec![f];
                            if name == "max" {
                                a.push(Expr::Bool(true));
                            }
                            return Ok(Expr::Method(Box::new(Expr::Method(Box::new(xs), "sort_by".into(), a, line)), "first_or".into(), vec![d], line));
                        }
                        return Ok(Expr::Method(Box::new(xs), format!("{name}_or"), vec![d], line));
                    }
                    // min(xs, key=f) and max(xs, key=f): the first item with the smallest or largest key.
                    if (name == "min" || name == "max") && !self.user_fns.contains(&name) && args.len() == 2 && args[0].0.is_none() && args[1].0.as_deref() == Some("key") {
                        let f = args.remove(1).1;
                        let xs = args.remove(0).1;
                        let mut a = vec![f];
                        if name == "max" {
                            a.push(Expr::Bool(true));
                        }
                        return Ok(Expr::Index(Box::new(Expr::Method(Box::new(xs), "sort_by".into(), a, line)), Box::new(Expr::Int(0))));
                    }
                    // sorted(xs, key=f, reverse=r)
                    if name == "sorted" && !self.user_fns.contains(&name) && args.first().is_some_and(|a| a.0.is_none()) {
                        let recv = args[0].1.clone();
                        if let Some(s) = sort_kw(recv, &args[1..], line) {
                            return Ok(s);
                        }
                    }
                    // range(n), range(a, b) and range(a, b, step) are a..b.
                    if name == "range" && (1..=3).contains(&args.len()) && args.iter().all(|a| a.0.is_none()) && !self.user_fns.contains(&name) {
                        let mut a: Vec<Expr> = args.into_iter().map(|a| a.1).collect();
                        let (lo, hi) = if a.len() == 1 { (Expr::Int(0), a.remove(0)) } else { (a.remove(0), a.remove(0)) };
                        // range(a, b, -k) counts down: a, a-k, ... while above b, which is (b+1..a+1) reversed, every k-th.
                        let neg = match a.first() {
                            Some(Expr::Unary("-", x)) => if let Expr::Int(k) = x.as_ref() { Some(*k) } else { None },
                            Some(Expr::Int(k)) if *k < 0 => Some(-k),
                            _ => None,
                        };
                        if let Some(k) = neg {
                            let one = |e: Expr| Expr::Binary("+", Box::new(e), Box::new(Expr::Int(1)));
                            let r = Expr::Method(Box::new(Expr::Range(Box::new(one(hi)), Box::new(one(lo)))), "rev".into(), vec![], line);
                            return Ok(if k == 1 { r } else { Expr::Method(Box::new(r), "step".into(), vec![Expr::Int(k)], line) });
                        }
                        let r = Expr::Range(Box::new(lo), Box::new(hi));
                        return Ok(if a.is_empty() { r } else { Expr::Method(Box::new(r), "step".into(), a, line) });
                    }
                    // any(c for x in xs) and all(...) test each item.
                    if (name == "any" || name == "all") && args.len() == 1 && !self.user_fns.contains(&name) {
                        let arg = args.remove(0).1;
                        return Ok(match arg {
                            Expr::Method(base, mm, mut la, _) if mm == "map" && la.len() == 1 => {
                                let f = la.remove(0);
                                // `all(parts[i] for i in ...)`: each value by its truth.
                                let f = match f {
                                    Expr::Lambda(ps, body) if self.py && !is_bool_expr(&body) => Expr::Lambda(ps, Box::new(Expr::Unary("!", Box::new(Expr::Unary("!", body))))),
                                    f => f,
                                };
                                Expr::Method(base, name, vec![f], line)
                            }
                            // Python's `all(words)` tests each item's truth (`!!` is bool(x)).
                            other => {
                                let b = Expr::Ident("b_".into());
                                let b = if self.py { Expr::Unary("!", Box::new(Expr::Unary("!", Box::new(b)))) } else { b };
                                Expr::Method(Box::new(other), name, vec![Expr::Lambda(vec!["b_".into()], Box::new(b))], line)
                            }
                        });
                    }
                    // defaultdict(lambda: 0) (read as a `lambda:` named argument) with an empty default is a map.
                    if name == "defaultdict" && args.len() == 1 && args[0].0.as_deref() == Some("lambda") && empty_literal(&args[0].1) && !self.user_fns.contains(&name) {
                        return Ok(lax_map(line));
                    }
                    // enumerate(xs, start=1)
                    if name == "enumerate" && args.len() == 2 && args[1].0.as_deref() == Some("start") && !self.user_fns.contains(&name) {
                        let n = args.remove(1).1;
                        return Ok(Expr::Method(Box::new(args.remove(0).1), "enumerate_from".into(), vec![n], line));
                    }
                    // itertools.groupby(xs, key=f): runs of neighbours with the same key, as (key, items) pairs.
                    if name == "groupby" && !self.user_fns.contains(&name) && (args.len() == 1 || (args.len() == 2 && args[1].0.as_deref() == Some("key"))) {
                        let xs = args.remove(0).1;
                        let f = if args.is_empty() {
                            Expr::Lambda(vec!["v_".into()], Box::new(Expr::Ident("v_".into())))
                        } else {
                            args.remove(0).1
                        };
                        return Ok(Expr::Method(Box::new(xs), "group_runs".into(), vec![f], line));
                    }
                    // timedelta(days=n, weeks=w) is a span of days that adds to a date.
                    if name == "timedelta" && !args.is_empty() && !self.user_fns.contains(&name) && args.iter().all(|a| matches!(a.0.as_deref(), None | Some("days") | Some("weeks"))) {
                        let mut total: Option<Expr> = None;
                        for (n, v) in args.drain(..) {
                            let v = if n.as_deref() == Some("weeks") { Expr::Binary("*", Box::new(v), Box::new(Expr::Int(7))) } else { v };
                            total = Some(match total {
                                Some(t) => Expr::Binary("+", Box::new(t), Box::new(v)),
                                None => v,
                            });
                        }
                        return Ok(Expr::Method(Box::new(total.unwrap()), "delta_days".into(), vec![], line));
                    }
                    // print(..., file=sys.stderr): the same line, written to stderr.
                    if name == "print" && self.py && !self.user_fns.contains(&name) {
                        let to_out = |v: &Expr| matches!(v, Expr::Field(m, f) if f == "stdout" && matches!(m.as_ref(), Expr::Ident(s) if s == "sys"));
                        args.retain(|(n, v)| !(n.as_deref() == Some("file") && to_out(v)));
                        let to_err = |v: &Expr| matches!(v, Expr::Field(m, f) if f == "stderr" && matches!(m.as_ref(), Expr::Ident(s) if s == "sys"));
                        if let Some(i) = args.iter().position(|(n, v)| n.as_deref() == Some("file") && to_err(v)) {
                            args.remove(i);
                            let sep = args.iter().find(|(n, _)| n.as_deref() == Some("sep")).map(|(_, v)| v.clone()).unwrap_or(Expr::Str(vec![StrPart::Lit(" ".into())]));
                            let end = args.iter().find(|(n, _)| n.as_deref() == Some("end")).map(|(_, v)| v.clone()).unwrap_or(Expr::Str(vec![StrPart::Lit("\n".into())]));
                            let items: Vec<Expr> = args.iter().filter(|(n, _)| n.is_none()).map(|(_, v)| Expr::Call("str".into(), vec![(None, v.clone())], line)).collect();
                            let body = Expr::Method(Box::new(Expr::List(items)), "join".into(), vec![sep], line);
                            return Ok(Expr::Call("__eprint".into(), vec![(None, Expr::Binary("+", Box::new(body), Box::new(end)))], line));
                        }
                    }
                    // open(path, mode=..., encoding=...): keyword arguments too.
                    if name == "open" && self.py && !self.user_fns.contains(&name) {
                        if let Some(d) = python_call(&name, &mut args, line) {
                            return Ok(d);
                        }
                    }
                    let star = args.iter().any(|a| a.0.as_deref() == Some("__star"));
                    if !self.user_fns.contains(&name) && !self.class_names.contains(&name) && (star || args.iter().all(|a| a.0.is_none())) {
                        if let Some(d) = python_call(&name, &mut args, line) {
                            return Ok(d);
                        }
                    }
                    match m {
                        Some(m) if args.len() == 1 && args[0].0.is_none() && !self.user_fns.contains(&name) => Ok(Expr::Method(Box::new(args.remove(0).1), m.into(), vec![], line)),
                        // A program's own function named like a built-in (`fn args(...)`) is its own.
                        _ if self.user_fns.contains(&name) && SHADOWABLE.contains(&name.as_str()) => Ok(Expr::Call(format!("{name}_"), args, line)),
                        _ => Ok(Expr::Call(name, args, line)),
                    }
                } else if matches!(name.as_str(), "int" | "float" | "str" | "len") && !self.is_sym("=") {
                    // `map(int, xs)`, `key=len`: a built-in conversion used as a function value.
                    let v = Expr::Ident("v_".into());
                    Ok(Expr::Lambda(vec!["v_".into()], Box::new(Expr::Method(Box::new(v), name, vec![], line))))
                } else if let Some(x) = crate::pybridge::ident(&name, line) {
                    Ok(x)
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            Tok::Sym("(") => {
                let mut items = Vec::new();
                while !self.is_sym(")") {
                    items.push(self.expr()?);
                    // `(x * 2 for x in xs)`: a generator is the list it makes.
                    if items.len() == 1 && self.eat_kw("for") {
                        let c = self.comprehension(items.pop().unwrap())?;
                        self.expect_sym(")")?;
                        return Ok(c);
                    }
                    if !self.eat_sym(",") {
                        break;
                    }
                    // Python's `(x,)`: a one-item tuple, kept as a one-item list.
                    if items.len() == 1 && self.is_sym(")") {
                        self.next();
                        return Ok(Expr::List(items));
                    }
                }
                self.expect_sym(")")?;
                if items.len() == 1 {
                    Ok(items.pop().unwrap())
                } else if items.is_empty() && self.py {
                    // Python's `()` is an empty sequence (`adj.get(n, ())`).
                    Ok(Expr::List(items))
                } else {
                    Ok(Expr::Tuple(items))
                }
            }
            Tok::Sym("[") => {
                let mut items = Vec::new();
                while !self.is_sym("]") {
                    let flat = self.eat_sym("*");
                    let e = self.expr()?;
                    // `[f(x) for x in xs if c]`
                    if self.eat_kw("for") {
                        let c = self.flat_comprehension(e, flat)?;
                        self.expect_sym("]")?;
                        return Ok(c);
                    }
                    if flat {
                        return Err(self.err("expected for after *x").fix_hint("join lists with +: a + b"));
                    }
                    items.push(e);
                    if !self.eat_sym(",") {
                        break;
                    }
                }
                self.expect_sym("]")?;
                Ok(Expr::List(items))
            }
            Tok::Sym("{") => {
                // In expression position braces are always a map literal.
                let mut pairs = Vec::new();
                self.skip_seps();
                while !self.is_sym("}") {
                    let k = self.expr()?;
                    // Python lets a `for` or `:` start the next line inside braces.
                    self.skip_newlines();
                    // Python's sets: `{a, b}` and `{w for w in ws}` are lists without repeats.
                    if pairs.is_empty() && !self.is_sym(":") {
                        let line = self.line();
                        let list = if self.eat_kw("for") {
                            self.comprehension(k)?
                        } else {
                            let mut xs = vec![k];
                            while self.eat_sym(",") {
                                self.skip_newlines();
                                if self.is_sym("}") {
                                    break;
                                }
                                xs.push(self.expr()?);
                            }
                            Expr::List(xs)
                        };
                        self.skip_newlines();
                        self.expect_sym("}")?;
                        return Ok(Expr::Method(Box::new(list), "unique".into(), vec![], line));
                    }
                    self.expect_sym(":")?;
                    self.skip_newlines();
                    let v = self.expr()?;
                    self.skip_newlines();
                    // Python's `{k: v for x in xs}`: a map filled by a loop over the pairs.
                    if pairs.is_empty() && self.is_kw("for") {
                        let line = self.line();
                        self.next();
                        let list = self.comprehension(Expr::Tuple(vec![k, v]))?;
                        self.skip_newlines();
                        self.expect_sym("}")?;
                        // `{c: i for i, c in enumerate(s)}`: the pairs made into a map, typed from the pairs.
                        return Ok(Expr::Method(Box::new(list), "to_dict".into(), vec![], line));
                    }
                    pairs.push((k, v));
                    self.skip_seps();
                }
                self.expect_sym("}")?;
                Ok(Expr::Map(pairs))
            }
            Tok::Kw("if") => {
                let cond = self.expr()?;
                let then = self.block()?;
                let save = self.pos;
                self.skip_newlines();
                if self.eat_kw("else") {
                    let other = if self.is_kw("if") { self.primary()? } else { self.block()? };
                    Ok(Expr::If(Box::new(cond), Box::new(then), Some(Box::new(other))))
                } else {
                    self.pos = save;
                    Ok(Expr::If(Box::new(cond), Box::new(then), None))
                }
            }
            Tok::Kw("match") => {
                let subject = self.expr()?;
                // Inside brackets the arms are not indented into a block: `(match v: A => 1  _ => 0)`.
                if self.is_sym(":") {
                    self.pos += 1;
                    let mut arms = Vec::new();
                    self.skip_seps();
                    while !self.is_sym(")") && !self.is_sym("]") && self.pos < self.toks.len() {
                        let p = self.pat()?;
                        self.expect_sym("=>")?;
                        let body = self.expr()?;
                        arms.push((p, body));
                        if self.is_sym(",") {
                            self.pos += 1;
                        }
                        self.skip_seps();
                    }
                    return Ok(Expr::Match(Box::new(subject), arms));
                }
                self.expect_sym("{")?;
                let mut arms = Vec::new();
                self.skip_seps();
                while !self.is_sym("}") {
                    let p = self.pat()?;
                    // `Arr(xs):` as in Python's `case`, for `Arr(xs) =>`.
                    if !self.eat_sym(":") && !self.is_sym("{") {
                        self.expect_sym("=>")?;
                    }
                    self.skip_newlines();
                    let next = self.toks.get(self.pos + 1).map(|t| t.tok.clone());
                    let after = self.toks.get(self.pos + 2).map(|t| t.tok.clone());
                    let map = self.is_sym("{")
                        && match (&next, &after) {
                            (Some(Tok::Sym("}")), _) => self.ret_map,
                            (Some(Tok::Str(_) | Tok::Int(_) | Tok::Ident(_)), Some(Tok::Sym(":"))) => true,
                            _ => false,
                        };
                    let body = if map {
                        self.expr()?
                    } else if self.is_sym("{") {
                        self.block()?
                    } else {
                        // An arm can also be one statement, such as `Num(n) => total += n`.
                        match self.stmt()? {
                            Stmt::Expr { expr, .. } => expr,
                            st => Expr::Block(vec![st], None),
                        }
                    };
                    arms.push((p, body));
                    self.skip_seps();
                }
                self.expect_sym("}")?;
                // `match run(x):` with only an `Err(m)` arm: success needs nothing done.
                let has = |n: &str| arms.iter().any(|(p, _)| matches!(p, Pat::Variant(v, _) if v == n));
                if has("Err") && !has("Ok") && !arms.iter().any(|(p, _)| matches!(p, Pat::Wild | Pat::Bind(_))) {
                    arms.push((Pat::Variant("Ok".into(), vec![Pat::Wild]), Expr::Block(vec![], None)));
                }
                Ok(Expr::Match(Box::new(subject), arms))
            }
            Tok::Kw("for") => {
                let mut p = bind_upper(self.pat()?);
                // `for k, v in ...` unpacks like `for (k, v) in ...`.
                if self.is_sym(",") {
                    let mut ps = vec![p];
                    while self.eat_sym(",") {
                        ps.push(bind_upper(self.pat()?));
                    }
                    p = Pat::Tuple(ps);
                }
                if !self.eat_kw("in") {
                    return Err(self.err("expected 'in' after the loop variable"));
                }
                let mut iter = self.expr()?;
                crate::pybridge::note_for(&p, &iter);
                // `for line in open(p)`: the file's lines, each with its "\n".
                if let Expr::Call(f, a, l) = &iter {
                    if f == "__open_r" {
                        iter = Expr::Method(Box::new(Expr::Call("read".into(), a.clone(), *l)), "lines_keep".into(), vec![], *l);
                    }
                }
                let body = self.block()?;
                let lp = Expr::For(p, Box::new(iter), Box::new(body));
                self.loop_else(lp)
            }
            Tok::Kw("while") => {
                let cond = self.expr()?;
                let body = self.block()?;
                let lp = Expr::While(Box::new(cond), Box::new(body));
                self.loop_else(lp)
            }
            t => {
                self.pos -= 1;
                Err(self.err(&format!("expected an expression, found {}", tok_text(&t))))
            }
        }
    }

    // Python's `for ...: ... else: ...` runs the else part when the loop ends without `break`.
    fn loop_else(&mut self, lp: Expr) -> R<Expr> {
        let save = self.pos;
        self.skip_newlines();
        if !(self.is_kw("else") && matches!(self.peek_at(1), Tok::Sym(":" | "{"))) {
            self.pos = save;
            return Ok(lp);
        }
        self.next();
        let other = self.block()?;
        let line = self.line();
        let flag = format!("ax_broke{}", self.pos);
        fn mark(e: &mut Expr, flag: &str, line: usize) {
            match e {
                Expr::Block(st, tail) => {
                    let mut out = Vec::with_capacity(st.len());
                    for mut s in std::mem::take(st) {
                        match &mut s {
                            Stmt::Break { .. } => {
                                out.push(Stmt::Assign { target: Expr::Ident(flag.into()), op: "=", value: Expr::Bool(true), line });
                            }
                            Stmt::Expr { expr, .. } => mark(expr, flag, line),
                            _ => {}
                        }
                        out.push(s);
                    }
                    *st = out;
                    if let Some(t) = tail {
                        mark(t, flag, line);
                    }
                }
                Expr::If(_, a, b) => {
                    mark(a, flag, line);
                    if let Some(b) = b {
                        mark(b, flag, line);
                    }
                }
                Expr::Match(_, arms) => arms.iter_mut().for_each(|(_, a)| mark(a, flag, line)),
                _ => {}
            }
        }
        let mut lp = lp;
        match &mut lp {
            Expr::For(_, _, b) | Expr::While(_, b) => mark(b, &flag, line),
            _ => {}
        }
        Ok(Expr::Block(
            vec![
                Stmt::Assign { target: Expr::Ident(flag.clone()), op: "=", value: Expr::Bool(false), line },
                Stmt::Expr { expr: lp, line },
                Stmt::Expr { expr: Expr::If(Box::new(Expr::Unary("!", Box::new(Expr::Ident(flag)))), Box::new(other), None), line },
            ],
            None,
        ))
    }

    // Splits "a {x} b" into literal and interpolated parts. "{{" is a literal brace.
    fn interp(&self, s: &str, line: usize) -> R<Vec<StrPart>> {
        let mut parts = Vec::new();
        let mut lit = String::new();
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '{' if chars.get(i + 1) == Some(&'{') => {
                    lit.push('{');
                    i += 2;
                }
                '}' if chars.get(i + 1) == Some(&'}') => {
                    lit.push('}');
                    i += 2;
                }
                // In Python files a lone `{` only reaches here from an f-string (the lexer doubles the braces of
                // plain strings), so `{1/3:.3f}` and `{2*n}` are values too.
                '{' if !chars.get(i + 1).map_or(false, |c| c.is_alphabetic() || matches!(c, '_' | '(' | '[' | '-' | '!' | '"' | '\'') || (self.py && (c.is_ascii_digit() || *c == '.'))) => {
                    lit.push('{');
                    i += 1;
                }
                '{' => {
                    let mut depth = 1;
                    let start = i + 1;
                    i += 1;
                    while i < chars.len() && depth > 0 {
                        match chars[i] {
                            '{' => depth += 1,
                            '}' => depth -= 1,
                            _ => {}
                        }
                        i += 1;
                    }
                    if depth > 0 {
                        return Err(Diag::new("parse", line, 1, "unclosed '{' in string").fix_hint("use {{ for a literal brace"));
                    }
                    let inner: String = chars[start..i - 1].iter().collect();
                    // Python's format spec: `{x:.2f}`, `{n:03d}`, `{s:<10}`, `{v:>8}`.
                    let (inner, spec) = split_spec(&inner, self.py);
                    // Python's `{x!r}` / `{x!s}` / `{x!a}` conversions and `{x=}` (the text, then the value).
                    let (mut inner, mut conv, mut debug) = (inner, None::<char>, false);
                    if self.py {
                        let t = inner.trim_end();
                        if let Some(c) = ["!r", "!s", "!a"].iter().find(|c| t.ends_with(**c)) {
                            conv = c.chars().nth(1);
                            inner = t[..t.len() - 2].to_string();
                        }
                        let t = inner.trim_end();
                        if t.ends_with('=') && !t.ends_with("==") && !t.ends_with("!=") && !t.ends_with("<=") && !t.ends_with(">=") {
                            debug = true;
                            lit.push_str(&inner);
                            inner = t[..t.len() - 1].to_string();
                        }
                    }
                    let toks = lex(&inner).map_err(|mut d| {
                        d.line = line;
                        d
                    })?;
                    let mut sub = Parser { toks, pos: 0, ret_map: false, user_fns: self.user_fns.clone(), children: vec![], hoisted: vec![], pending: vec![], set_names: vec![], py: self.py, sigs: HashMap::new(), class_names: vec![], held: vec![], yields: false, attr_tys: vec![], type_names: HashMap::new(), exc_bases: self.exc_bases.clone(), in_handler: 0, exc_rich: self.exc_rich.clone(), threads: self.threads.clone(), in_exc_class: false, exc_init_arg: None };
                    let e = sub.expr().map_err(|mut d| {
                        d.line = line;
                        d
                    })?;
                    if self.py {
                        sub.skip_newlines();
                        if sub.peek() != &Tok::Eof {
                            return Err(Diag::new("parse", line, 1, &format!("cannot read the f-string value {{{inner}}}")).fix_hint("compute the value first: v = ...; then f\"{v}\""));
                        }
                    }
                    if !lit.is_empty() {
                        parts.push(StrPart::Lit(std::mem::take(&mut lit)));
                    }
                    let e = match conv {
                        Some('r' | 'a') => Expr::Method(Box::new(e), "py_repr".into(), vec![], line),
                        Some(_) => Expr::Call("str".into(), vec![(None, e)], line),
                        None if debug && spec.is_none() => Expr::Method(Box::new(e), "py_repr".into(), vec![], line),
                        None => e,
                    };
                    let e = match spec {
                        // `{x:{w}.2f}`: the spec is itself filled in.
                        Some(sp) if self.py && sp.contains('{') => Expr::Method(Box::new(e), "spec".into(), vec![Expr::Str(self.interp(&sp, line)?)], line),
                        Some(sp) => Expr::Method(Box::new(e), "spec".into(), vec![Expr::Str(vec![StrPart::Lit(sp)])], line),
                        None => e,
                    };
                    parts.push(StrPart::Interp(e));
                }
                c => {
                    lit.push(c);
                    i += 1;
                }
            }
        }
        if !lit.is_empty() || parts.is_empty() {
            parts.push(StrPart::Lit(lit));
        }
        Ok(parts)
    }
}

// A declared name like `MAX = 10` is a variable even though it is capitalised.
// Python method names models write by habit, as the Brevi method they mean.
// Built-in functions a program may define for itself; its own version gets a suffix in the Rust.
thread_local! {
    // Dataclasses: class -> (its Python name, the fields its __repr__ shows).
    static DATACLASSES: std::cell::RefCell<std::collections::HashMap<String, (String, Vec<String>)>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

pub fn dataclass(name: &str) -> Option<(String, Vec<String>)> {
    DATACLASSES.with(|d| d.borrow().get(name).cloned())
}

const SHADOWABLE: &[&str] = &["args", "input", "stdin", "lines", "read", "write", "json", "csv", "date", "heap", "abs", "min", "max"];

// A whole Python format spec: [[fill]align][sign][z][#][0][width][,|_][.precision][type].
pub fn py_spec_ok(spec: &str) -> bool {
    let c: Vec<char> = spec.chars().collect();
    if c.iter().any(|&ch| ch == '{' || ch == '}') {
        return false;
    }
    let mut i = 0;
    if c.len() >= 2 && matches!(c[1], '<' | '>' | '^' | '=') {
        i = 2;
    } else if !c.is_empty() && matches!(c[0], '<' | '>' | '^' | '=') {
        i = 1;
    }
    if i < c.len() && matches!(c[i], '+' | '-' | ' ') {
        i += 1;
    }
    for flag in ['z', '#', '0'] {
        if i < c.len() && c[i] == flag {
            i += 1;
        }
    }
    while i < c.len() && c[i].is_ascii_digit() {
        i += 1;
    }
    if i < c.len() && matches!(c[i], ',' | '_') {
        i += 1;
    }
    if i < c.len() && c[i] == '.' {
        i += 1;
        let st = i;
        while i < c.len() && c[i].is_ascii_digit() {
            i += 1;
        }
        if i == st {
            return false;
        }
    }
    if i < c.len() && "bcdeEfFgGnosxX%".contains(c[i]) {
        i += 1;
    }
    i == c.len()
}

// `x:.2f` -> ("x", Some(".2f")): a colon outside brackets and quotes followed by a Python format spec.
fn split_spec(inner: &str, py: bool) -> (String, Option<String>) {
    let (mut depth, mut quote) = (0i32, None::<char>);
    for (i, c) in inner.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(' | '[' | '{') => depth += 1,
            (None, ')' | ']' | '}') => depth -= 1,
            (None, ':') if depth == 0 => {
                let spec = &inner[i + 1..];
                let ok = !spec.is_empty() && {
                    let b = spec.trim_start_matches(['<', '>', '^']).trim_start_matches('+');
                    let b = b.trim_start_matches(|c: char| c.is_ascii_digit() || c == ',');
                    let b = b.strip_prefix('.').map_or(b, |r| r.trim_start_matches(|c: char| c.is_ascii_digit()));
                    b.is_empty() || matches!(b, "d" | "f" | "s" | "x" | "%" | "e")
                } || py && (py_spec_ok(spec) || spec.contains('{'));
                return if ok { (inner[..i].to_string(), Some(spec.to_string())) } else { (inner.to_string(), None) };
            }
            _ => {}
        }
    }
    (inner.to_string(), None)
}

// Python's sort keywords: `key=f` and `reverse=r` (also `sort_by(f, reverse=r)`) become `sort_by(f, r)`.
fn sort_kw(recv: Expr, args: &[(Option<String>, Expr)], line: usize) -> Option<Expr> {
    let get = |k: &str| args.iter().find(|a| a.0.as_deref() == Some(k)).map(|a| a.1.clone());
    let (key, rev) = (get("key"), get("reverse"));
    if key.is_none() && rev.is_none() {
        return None;
    }
    let f = key
        .or_else(|| args.iter().find(|a| a.0.is_none()).map(|a| a.1.clone()))
        .unwrap_or_else(|| Expr::Lambda(vec!["x_".into()], Box::new(Expr::Ident("x_".into()))));
    let mut a = vec![f];
    a.extend(rev);
    Some(Expr::Method(Box::new(recv), "sort_by".into(), a, line))
}

// Renames a top-level `main()` call (plain, or in an if's blocks) to `main_()`; true when there was one.
fn rename_main(e: &mut Expr) -> bool {
    match e {
        Expr::Call(f, a, _) if f == "main" && a.is_empty() => {
            *f = "main_".into();
            true
        }
        Expr::If(_, a, b) => {
            let x = rename_main(a);
            b.as_mut().map_or(false, |b| rename_main(b)) || x
        }
        Expr::Block(stmts, tail) => {
            let mut hit = false;
            for st in stmts.iter_mut() {
                if let Stmt::Expr { expr, .. } = st {
                    hit |= rename_main(expr);
                }
            }
            tail.as_mut().map_or(false, |t| rename_main(t)) || hit
        }
        _ => false,
    }
}

// math functions as the typed build's float methods (receiver: the first argument), by argument count.
pub(crate) fn math_method(name: &str, nargs: usize) -> Option<&'static str> {
    Some(match (name, nargs) {
        ("log", 1) => "m_log",
        ("log", 2) => "m_logb",
        ("log2", 1) => "m_log2",
        ("log10", 1) => "m_log10",
        ("log1p", 1) => "m_log1p",
        ("exp", 1) => "m_exp",
        ("expm1", 1) => "m_expm1",
        ("sin", 1) => "m_sin",
        ("cos", 1) => "m_cos",
        ("tan", 1) => "m_tan",
        ("asin", 1) => "m_asin",
        ("acos", 1) => "m_acos",
        ("atan", 1) => "m_atan",
        ("atan2", 2) => "m_atan2",
        ("sinh", 1) => "m_sinh",
        ("cosh", 1) => "m_cosh",
        ("tanh", 1) => "m_tanh",
        ("hypot", 2) => "m_hypot",
        ("fabs", 1) => "m_fabs",
        ("trunc", 1) => "m_trunc",
        ("degrees", 1) => "m_degrees",
        ("radians", 1) => "m_radians",
        ("copysign", 2) => "m_copysign",
        ("fmod", 2) => "m_fmod",
        ("isnan", 1) => "m_isnan",
        ("isinf", 1) => "m_isinf",
        ("isfinite", 1) => "m_isfinite",
        _ => return None,
    })
}

// Python's heapq functions (`from heapq import heappush` or `heapq.heappush`) as list methods.
fn heapq_method(name: &str, nargs: usize) -> Option<&'static str> {
    Some(match (name, nargs) {
        ("heappush", 2) => "heap_push",
        ("heappop", 1) => "heap_pop",
        ("heapify", 1) => "heapify",
        ("heappushpop", 2) => "heap_push_pop",
        ("heapreplace", 2) => "heap_replace",
        _ => return None,
    })
}

fn python_method(name: &str, nargs: usize) -> Option<String> {
    Some(match (name, nargs) {
        ("append", 1) => "push",
        ("sorted", 0) => "sort",
        ("reversed", 0) => "rev",
        ("strip", 0) => "trim",
        ("strip", 1) => "trim_of",
        ("rstrip", 0) => "trim_end",
        ("rstrip", 1) => "trim_end_of",
        ("lstrip", 0) => "trim_start",
        ("lstrip", 1) => "trim_start_of",
        ("splitlines", 0) => "lines",
        ("splitlines", 1) => "lines_keep",
        // xs.pop(i) and m.pop(k) take the item out and give it back.
        ("pop", 1) => "pop_at",
        // m.pop(k, d): the value taken out, or d when k is missing.
        ("pop", 2) => "pop_or",
        ("isupper", 0) => "is_upper",
        ("islower", 0) => "is_lower",
        ("isalnum", 0) => "is_alnum",
        ("isascii", 0) => "is_ascii",
        // d.to_integral_value() rounds a Decimal half to even, as Python's default context does.
        ("to_integral_value" | "to_integral", 0) => "round",
        ("center", 2) => "center_with",
        ("ljust", 2) => "ljust_with",
        ("rjust", 2) => "rjust_with",
        ("most_common", 0) => "most_common_all",
        ("split", 0) => "words",
        // `s.casefold()`: lower case (enough for comparing words without regard to case).
        ("casefold", 0) => "lower",
        ("__floor__", 0) => "floor",
        ("__ceil__", 0) => "ceil",
        ("__abs__", 0) => "abs",
        ("split", 2) => "split_n",
        ("rsplit", 2) => "rsplit_n",
        ("startswith", 1) => "starts_with",
        ("startswith", 2) => "starts_with_at",
        ("endswith", 2) => "ends_with_at",
        ("endswith", 1) => "ends_with",
        ("isdigit", 0) => "is_digit",
        ("isalpha", 0) => "is_alpha",
        ("isspace", 0) => "is_space",
        ("items", 0) => "pairs",
        ("index", 1) => "index_of",
        ("get", 2) => "get_or",
        ("char_at" | "to_char" | "chr", 0) => "char",
        ("find", 2) => "find_from",
        _ => return None,
    }
    .into())
}

// Python built-ins spelled as Brevi: set(xs), divmod(a, b), round(x, n), enumerate(xs, 1), defaultdict(int).
fn python_call(name: &str, args: &mut Vec<(Option<String>, Expr)>, line: usize) -> Option<Expr> {
    let m = |e: Expr, n: &str, a: Vec<Expr>| Expr::Method(Box::new(e), n.into(), a, line);
    if args.iter().any(|(n, _)| n.as_deref() == Some("__star")) {
        // `date(*map(int, s.split("-")))`: the three parts joined back into "y-m-d".
        if name == "date" && args.len() == 1 {
            let e = args.remove(0).1;
            let parts = m(e, "map", vec![Expr::Lambda(vec!["ax_x".into()], Box::new(Expr::Call("str".into(), vec![(None, Expr::Ident("ax_x".into()))], line)))]);
            return Some(Expr::Call("date".into(), vec![(None, m(parts, "join", vec![Expr::Str(vec![StrPart::Lit("-".into())])]))], line));
        }
        if name != "print" {
            return None;
        }
        // `print(*xs, 1)`: the items written with the separator between them.
        let mut items: Option<Expr> = None;
        let mut rest = Vec::new();
        for (n, e) in args.drain(..) {
            let part = match n.as_deref() {
                Some("__star") => m(e, "map", vec![Expr::Lambda(vec!["ax_x".into()], Box::new(Expr::Call("str".into(), vec![(None, Expr::Ident("ax_x".into()))], line)))]),
                None => Expr::List(vec![Expr::Call("str".into(), vec![(None, e)], line)]),
                Some(_) => {
                    rest.push((n, e));
                    continue;
                }
            };
            items = Some(match items {
                Some(a) => Expr::Binary("+", Box::new(a), Box::new(part)),
                None => part,
            });
        }
        let sep = rest.iter().position(|(n, _)| n.as_deref() == Some("sep")).map(|i| rest.remove(i).1).unwrap_or(Expr::Str(vec![StrPart::Lit(" ".into())]));
        let joined = m(items.unwrap_or(Expr::List(vec![])), "join", vec![sep]);
        let mut a = vec![(None, joined)];
        a.extend(rest);
        return Some(Expr::Call("print".into(), a, line));
    }
    // open(path, mode): "__open_r" / "__open_w" / "__open_a" with the path; fix_opens turns the file into its text
    // (reading) or a list of written pieces that is written out at close (writing). Binary modes are not handled.
    if name == "open" && !args.is_empty() && args[0].0.is_none() {
        let mode_at = args.iter().position(|(n, _)| n.as_deref() == Some("mode")).or(if args.len() > 1 && args[1].0.is_none() { Some(1) } else { None });
        let mode = match mode_at.map(|i| &args[i].1) {
            None => "r".to_string(),
            Some(Expr::Str(p)) => match p.as_slice() {
                [StrPart::Lit(m)] => m.clone(),
                [] => "r".to_string(),
                _ => return None,
            },
            Some(_) => return None,
        };
        if mode.contains('b') || mode.contains('+') || mode.contains('x') {
            return None;
        }
        let kind = if mode.contains('w') { "__open_w" } else if mode.contains('a') { "__open_a" } else { "__open_r" };
        let path = args.remove(0).1;
        return Some(Expr::Call(kind.into(), vec![(None, path)], line));
    }
    if let Some(hm) = heapq_method(name, args.len()) {
        let h = args.remove(0).1;
        return Some(m(h, hm, args.drain(..).map(|a| a.1).collect()));
    }
    Some(match (name, args.len()) {
        ("set" | "list" | "deque" | "frozenset", 0) => Expr::List(vec![]),
        ("frozenset", 1) => m(args.remove(0).1, "unique", vec![]),
        // repr(x) is the text Python shows for x inside a list; iter(xs) is xs (next() takes its first item).
        ("repr", 1) => m(args.remove(0).1, "py_repr", vec![]),
        ("iter", 1) => args.remove(0).1,
        ("Decimal", 1) => m(args.remove(0).1, "dec", vec![]),
        // next(rows) takes the first item, as reading one from an iterator does.
        ("next", 1) => match args.remove(0).1 {
            x @ Expr::Ident(_) => m(x, "pop_at", vec![Expr::Int(0)]),
            // next(x for x in xs if c): the first match.
            other => m(other, "first", vec![]),
        },
        // next(rows, None): the first item taken out, or the empty value when there is none.
        ("next", 2) => {
            let d = args.remove(1).1;
            let x = args.remove(0).1;
            if matches!(&d, Expr::Call(n, a, _) if n == "none" && a.is_empty()) {
                // Python's None itself, so `x is None` tests true afterwards.
                m(x, "next_or_none", vec![])
            } else {
                m(x, "next_or", vec![d])
            }
        }
        ("OrderedDict", 0) => Expr::Map(vec![]),
        // date(y, m, d) checks and builds the date as date("y-m-d") does.
        ("date", 3) => {
            let d = args.remove(2).1;
            let mo = args.remove(1).1;
            let y = args.remove(0).1;
            let dash = || StrPart::Lit("-".into());
            Expr::Call("date".into(), vec![(None, Expr::Str(vec![StrPart::Interp(y), dash(), StrPart::Interp(mo), dash(), StrPart::Interp(d)]))], line)
        }
        // zip(xs, ys), Counter(xs) (each item's count, in first-seen order) and dict(pairs).
        ("zip", 2) => {
            let ys = args.remove(1).1;
            m(args.remove(0).1, "zip", vec![ys])
        }
        ("Counter", 1) => match args.remove(0).1 {
            // Counter(s.split()) counts the words without building the list first.
            Expr::Method(s, w, a, _) if w == "words" && a.is_empty() => m(*s, "tally_words", vec![]),
            x => m(x, "tally", vec![]),
        },
        ("dict", 1) => m(args.remove(0).1, "to_dict", vec![]),
        // format(x, spec) is f"{x:spec}"; Fraction(s) and Fraction(a, b) are exact decimals.
        ("format", 1) => Expr::Call("str".into(), vec![(None, args.remove(0).1)], line),
        ("format", 2) => {
            let spec = args.remove(1).1;
            m(args.remove(0).1, "spec", vec![spec])
        }
        // A Fraction is an exact decimal marked to print as n/d (and to refuse 1/3 loudly).
        ("Fraction", 1) => m(m(args.remove(0).1, "dec", vec![]), "frac", vec![]),
        ("Fraction", 2) => {
            let b = m(args.remove(1).1, "dec", vec![]);
            m(m(args.remove(0).1, "dec", vec![]), "frac_div", vec![b])
        }

        // tuple(xs) is kept as a list: it sorts, compares and hashes the same way (it prints with [ ]).
        ("deque" | "list" | "tuple", 1) => m(args.remove(0).1, "skip", vec![Expr::Int(0)]),
        ("map" | "filter", 2) => {
            // `map(sum, rows)`, `map(abs, xs)`: a built-in used as the function value.
            let f = match args.remove(0).1 {
                Expr::Ident(b) if matches!(b.as_str(), "sum" | "abs" | "max" | "min" | "sorted") => {
                    let v = Expr::Ident("v_".into());
                    let body = match b.as_str() {
                        "abs" => Expr::Call(b, vec![(None, v)], line),
                        "sorted" => m(v, "sort", vec![]),
                        _ => m(v, &b, vec![]),
                    };
                    Expr::Lambda(vec!["v_".into()], Box::new(body))
                }
                f => f,
            };
            m(args.remove(0).1, name, vec![f])
        }
        ("dict", 0) => Expr::Map(vec![]),
        ("Counter", 0) | ("defaultdict", 1) => lax_map(line),
        ("set", 1) => m(args.remove(0).1, "unique", vec![]),
        // dict(m) copies a map; maps are values in Brevi already.
        ("OrderedDict", 1) => args.remove(0).1,
        // isinstance(v, int) / isinstance(v, (int, float)) asks the value's kind.
        ("isinstance", 2) => {
            // `int`, `float`, `str` alone read as conversion functions: v_.int() and so on.
            fn type_name(x: &Expr) -> Option<String> {
                match x {
                    Expr::Ident(n) => Some(n.clone()),
                    Expr::Lambda(_, b) => match b.as_ref() {
                        Expr::Method(_, n, a, _) if a.is_empty() => Some(n.clone()),
                        _ => None,
                    },
                    _ => None,
                }
            }
            let names: Vec<String> = match &args[1].1 {
                Expr::Tuple(xs) => xs.iter().filter_map(type_name).collect(),
                x => vec![type_name(x)?],
            };
            let names: Vec<String> = names.into_iter().map(|n| match n.as_str() {
                "set" | "List" => "list".to_string(),
                "Dict" | "OrderedDict" | "defaultdict" => "dict".to_string(),
                _ => n,
            }).collect();
            args.pop();
            m(args.remove(0).1, "isinstance", vec![Expr::Str(vec![StrPart::Lit(names.join(","))])])
        }
        // bool(x) is x's truth: not not x.
        // int(s, base): the text read in that base (base 0 reads a 0x, 0o or 0b prefix).
        ("int", 2) => {
            let b = args.remove(1).1;
            m(args.remove(0).1, "int_base", vec![b])
        }
        ("bool", 1) => Expr::Unary("!", Box::new(Expr::Unary("!", Box::new(args.remove(0).1)))),
        ("divmod", 2) => {
            let (a, b) = (args[0].1.clone(), args[1].1.clone());
            Expr::Tuple(vec![Expr::Binary("//", Box::new(a.clone()), Box::new(b.clone())), Expr::Binary("%", Box::new(a), Box::new(b))])
        }
        ("round", 1) => m(m(args.remove(0).1, "round", vec![]), "int", vec![]),
        ("round", 2) => {
            let n = args.remove(1).1;
            m(args.remove(0).1, "round", vec![n])
        }
        ("enumerate", 2) => {
            let n = args.remove(1).1;
            m(args.remove(0).1, "enumerate_from", vec![n])
        }
        _ => return None,
    })
}

// Fills `{}`/`{:spec}`/`{0}` (format) or `%s`/`%5.2f`/`%-8s` (percent) slots in a string's text
// with the arguments; None when the slots and arguments do not match up.
fn fill_format(parts: &[StrPart], args: Vec<Expr>, percent: bool, line: usize) -> Option<Vec<StrPart>> {
    let mut out = Vec::new();
    let mut lit = String::new();
    let mut next = 0;
    let mut used = vec![false; args.len()];
    for part in parts {
        let text = match part {
            StrPart::Lit(t) => t,
            other => {
                if !lit.is_empty() {
                    out.push(StrPart::Lit(std::mem::take(&mut lit)));
                }
                out.push(other.clone());
                continue;
            }
        };
        let cs: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < cs.len() {
            let slot = if percent && cs[i] == '%' {
                if cs.get(i + 1) == Some(&'%') {
                    lit.push('%');
                    i += 2;
                    continue;
                }
                let mut j = i + 1;
                while j < cs.len() && (cs[j].is_ascii_digit() || matches!(cs[j], '-' | '.' | '+')) {
                    j += 1;
                }
                let ty = *cs.get(j)?;
                if !matches!(ty, 'd' | 'i' | 's' | 'f' | 'r' | 'x' | 'e') {
                    return None;
                }
                let flags: String = cs[i + 1..j].iter().collect();
                let spec = match (ty, flags.strip_prefix('-')) {
                    (_, Some(w)) => format!("<{w}"),
                    ('s' | 'r', None) if !flags.is_empty() => format!(">{flags}"),
                    ('f' | 'x' | 'e', None) => format!("{flags}{ty}"),
                    (_, None) => flags,
                };
                i = j + 1;
                let k = next;
                next += 1;
                Some((k, spec))
            } else if !percent && cs[i] == '{' {
                let j = i + cs[i..].iter().position(|&c| c == '}')?;
                let inner: String = cs[i + 1..j].iter().collect();
                let (idx, spec) = inner.split_once(':').unwrap_or((&inner, ""));
                if !idx.chars().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                let k = if idx.is_empty() {
                    next += 1;
                    next - 1
                } else {
                    idx.parse().ok()?
                };
                i = j + 1;
                Some((k, spec.to_string()))
            } else {
                None
            };
            match slot {
                Some((k, spec)) => {
                    let a = args.get(k)?.clone();
                    *used.get_mut(k)? = true;
                    if !lit.is_empty() {
                        out.push(StrPart::Lit(std::mem::take(&mut lit)));
                    }
                    out.push(StrPart::Interp(if spec.is_empty() {
                        a
                    } else {
                        Expr::Method(Box::new(a), "spec".into(), vec![Expr::Str(vec![StrPart::Lit(spec)])], line)
                    }));
                }
                None => {
                    lit.push(cs[i]);
                    i += 1;
                }
            }
        }
    }
    if !lit.is_empty() {
        out.push(StrPart::Lit(lit));
    }
    used.iter().all(|u| *u).then_some(out)
}

// defaultdict(...) and Counter(): an empty map whose missing keys read as zero.
fn lax_map(line: usize) -> Expr {
    Expr::Method(Box::new(Expr::Map(vec![])), "lax".into(), vec![], line)
}

// `[]`, `{}`, `0`, `0.0`, `""` and `set()`/`list()`/`dict()`: the value a missing map key reads as.
fn empty_literal(e: &Expr) -> bool {
    match e {
        Expr::List(v) => v.is_empty(),
        Expr::Map(v) => v.is_empty(),
        Expr::Int(0) => true,
        Expr::Float(f) => *f == 0.0,
        Expr::Str(p) => p.is_empty(),
        _ => false,
    }
}

fn bind_upper(p: Pat) -> Pat {
    match p {
        Pat::Variant(n, args) if args.is_empty() => Pat::Bind(n),
        Pat::Tuple(ps) => Pat::Tuple(ps.into_iter().map(bind_upper).collect()),
        p => p,
    }
}

// Python: a function that returns a value on some paths (`return r, f` inside a loop) and can run off its end
// gives None there.
fn fall_off_none(body: &mut Expr, line: usize) {
    let mut has_value = false;
    walk_mut(body, &mut |e| {
        if let Expr::Block(ss, _) = e {
            has_value |= ss.iter().any(|s| matches!(s, Stmt::Return { value: Some(_), .. }));
        }
        // A lambda's body is its own function.
        if let Expr::Lambda(..) = e {}
    });
    let Expr::Block(stmts, tail) = body else { return };
    if !has_value {
        return;
    }
    // The block's last line is often held as its value (a loop that gives no value).
    if let Some(t) = tail.take() {
        if matches!(t.as_ref(), Expr::For(..) | Expr::While(..) | Expr::If(_, _, None)) {
            stmts.push(Stmt::Expr { expr: *t, line });
        } else {
            *tail = Some(t);
            return;
        }
    }
    let falls = match stmts.last() {
        Some(Stmt::Expr { expr: Expr::For(..), .. }) => true,
        Some(Stmt::Expr { expr: Expr::While(c, _), .. }) => !matches!(c.as_ref(), Expr::Bool(true)),
        Some(Stmt::Expr { expr: Expr::If(_, _, None), .. }) => true,
        _ => false,
    };
    if falls {
        *tail = Some(Box::new(Expr::Call("none".into(), vec![], line)));
    }
}

// Python's one-item list used as a box so nested defs can change it (`cur = [None]`, then `cur[0] = tx` and
// `tx = cur[0]` in the function and in `def finalize()`), read and written only as `cur[0]`: a plain name
// that the nested defs which set it declare `nonlocal`.
fn unbox_cells(body: &mut Expr, kids: &mut [Item]) {
    let Expr::Block(stmts, _) = body else { return };
    let cands: Vec<String> = stmts
        .iter()
        .filter_map(|st| match st {
            Stmt::Assign { target: Expr::Ident(n), op: "=", value: Expr::List(xs), .. } if xs.len() == 1 => Some(n.clone()),
            _ => None,
        })
        .collect();
    for x in cands {
        let count = |e: &mut Expr| {
            let (mut idents, mut cells, mut sets) = (0, 0, 0);
            walk_mut(e, &mut |e| match e {
                Expr::Ident(n) | Expr::Move(n) if *n == x => idents += 1,
                Expr::Index(b, k) if matches!(b.as_ref(), Expr::Ident(n) if *n == x) && matches!(k.as_ref(), Expr::Int(0)) => cells += 1,
                Expr::Block(ss, _) => {
                    for s in ss.iter() {
                        match s {
                            Stmt::Assign { target: Expr::Ident(n), .. } if *n == x => sets += 1,
                            Stmt::Let { name: Pat::Bind(n), .. } if *n == x => sets += 2,
                            _ => {}
                        }
                    }
                }
                _ => {}
            });
            (idents, cells, sets)
        };
        let (mut idents, mut cells, mut sets) = count(body);
        let mut in_kids = false;
        for k in kids.iter_mut() {
            if let Item::Fn { body: kb, params, .. } = k {
                let (i, c, st) = count(kb);
                idents += i;
                cells += c;
                sets += st;
                in_kids |= i > 0;
                if params.iter().any(|p| p.name == x) {
                    sets += 2;
                }
            }
        }
        if sets != 1 || idents != cells + 1 || !in_kids {
            continue;
        }
        let unbox = |e: &mut Expr| {
            walk_mut(e, &mut |e| {
                if matches!(e, Expr::Index(b, k) if matches!(b.as_ref(), Expr::Ident(n) if *n == x) && matches!(k.as_ref(), Expr::Int(0))) {
                    *e = Expr::Ident(x.clone());
                }
            });
        };
        let Expr::Block(stmts, _) = body else { return };
        for st in stmts.iter_mut() {
            if let Stmt::Assign { target: Expr::Ident(n), op: "=", value, .. } = st {
                if *n == x {
                    if let Expr::List(xs) = value {
                        let v = xs.remove(0);
                        *value = v;
                    }
                }
            }
        }
        unbox(body);
        for k in kids.iter_mut() {
            let Item::Fn { body: kb, line, .. } = k else { continue };
            unbox(kb);
            let mut assigns = false;
            walk_mut(kb, &mut |e| {
                if let Expr::Block(ss, _) = e {
                    assigns |= ss.iter().any(|s| matches!(s, Stmt::Assign { target: Expr::Ident(n), .. } if *n == x));
                }
            });
            if let (true, Expr::Block(ss, _)) = (assigns, kb) {
                ss.insert(0, Stmt::Expr { expr: Expr::Call("__global".into(), vec![(None, Expr::Ident(x.clone()))], *line), line: *line });
            }
        }
    }
}

// Python's `bad = lambda n: out.append(...)` in a function, set once and only ever called, is the nested
// `def bad(n): ...` (lifted with the rest), so its parameters are typed from the calls and it may change
// the outer function's lists.
// `d = rooms if kind == 'room' else items` with d then only used as `x in d` / `x not in d` (two dicts whose
// values differ): `d_pick = kind == 'room'` and each test asks the dict the condition picks.
fn pick_names(body: &mut Expr) {
    let bare = |x: &Expr| match x {
        Expr::Block(st, Some(v)) if st.is_empty() => v.as_ref().clone(),
        x => x.clone(),
    };
    let mut cands: Vec<(String, Expr, Expr)> = Vec::new();
    walk_mut(body, &mut |e| {
        if let Expr::Block(ss, _) = e {
            for st in ss.iter() {
                if let Stmt::Assign { target: Expr::Ident(n), op: "=", value: Expr::If(_, t, Some(f)), .. } = st {
                    // Plain names or fields (`w.rooms if kind == 'room' else w.items`).
                    let place = |x: &Expr| matches!(x, Expr::Ident(_)) || matches!(x, Expr::Field(r, _) if matches!(r.as_ref(), Expr::Ident(_)));
                    let (a, b) = (bare(t), bare(f));
                    if place(&a) && place(&b) && !matches!(&a, Expr::Ident(x) if x == n) && !matches!(&b, Expr::Ident(x) if x == n) {
                        cands.push((n.clone(), a, b));
                    }
                }
            }
        }
    });
    for (n, a, b) in cands {
        let (mut sets, mut idents, mut tests) = (0, 0, 0);
        walk_mut(body, &mut |e| match e {
            Expr::Ident(x) | Expr::Move(x) if *x == n => idents += 1,
            Expr::Binary("in" | "not in", _, r) if matches!(r.as_ref(), Expr::Ident(x) if *x == n) => tests += 1,
            Expr::Block(ss, _) => {
                for st in ss.iter() {
                    match st {
                        Stmt::Assign { target: Expr::Ident(x), .. } if *x == n => sets += 1,
                        Stmt::Let { name: Pat::Bind(x), .. } if *x == n => sets += 2,
                        _ => {}
                    }
                }
            }
            _ => {}
        });
        if sets != 1 || tests == 0 || idents != tests + sets {
            continue;
        }
        let flag = format!("{n}_pick");
        walk_mut(body, &mut |e| {
            if let Expr::Binary(op @ ("in" | "not in"), l, r) = e {
                if matches!(r.as_ref(), Expr::Ident(x) if *x == n) {
                    let op: &'static str = op;
                    let side = |d: &Expr| Expr::Block(vec![], Some(Box::new(Expr::Binary(op, l.clone(), Box::new(d.clone())))));
                    *e = Expr::If(Box::new(Expr::Ident(flag.clone())), Box::new(side(&a)), Some(Box::new(side(&b))));
                    return;
                }
            }
            if let Expr::Block(ss, _) = e {
                for st in ss.iter_mut() {
                    if let Stmt::Assign { target, op: "=", value: Expr::If(c, _, _), .. } = st {
                        if matches!(target, Expr::Ident(x) if *x == n) {
                            *target = Expr::Ident(flag.clone());
                            let c = c.as_ref().clone();
                            if let Stmt::Assign { value, .. } = st {
                                *value = c;
                            }
                        }
                    }
                }
            }
        });
    }
}

// `base = {'id': qid, 'points': n}` then `return dict(base, type='choice', answer=a)`: the dict written out
// (`{'id': qid, 'points': n, 'type': 'choice', 'answer': a}`), when base is set once from plain names and
// literals, so each shape is a record of its own.
fn dict_with_base(body: &mut Expr) {
    let simple = |v: &Expr| matches!(v, Expr::Ident(_) | Expr::Int(_) | Expr::Float(_) | Expr::Bool(_)) || matches!(v, Expr::Str(p) if matches!(p.as_slice(), [StrPart::Lit(_)] | []));
    let mut bases: Vec<(String, Vec<(Expr, Expr)>)> = Vec::new();
    let mut sets: HashMap<String, usize> = HashMap::new();
    walk_mut(body, &mut |e| {
        if let Expr::Block(ss, _) = e {
            for st in ss.iter() {
                if let Stmt::Assign { target: Expr::Ident(n), op, value, .. } = st {
                    *sets.entry(n.clone()).or_default() += 1;
                    if let (true, Expr::Map(es)) = (*op == "=", value) {
                        if !es.is_empty() && es.iter().all(|(k, v)| matches!(k, Expr::Str(p) if matches!(p.as_slice(), [StrPart::Lit(_)])) && simple(v)) {
                            bases.push((n.clone(), es.clone()));
                        }
                    }
                }
            }
        }
    });
    // The names the entries read must not change after base is set.
    bases.retain(|(n, es)| sets.get(n) == Some(&1) && es.iter().all(|(_, v)| !matches!(v, Expr::Ident(x) if sets.get(x).copied().unwrap_or(0) > 1)));
    if bases.is_empty() {
        return;
    }
    walk_mut(body, &mut |e| {
        if let Expr::Call(f, args, _) = e {
            if f != "dict" || args.len() < 2 || args[0].0.is_some() || args[1..].iter().any(|(k, _)| k.as_deref().map_or(true, |k| k == "__star")) {
                return;
            }
            let Expr::Ident(b) = &args[0].1 else { return };
            let Some((_, es)) = bases.iter().find(|(n, _)| n == b) else { return };
            let mut out = es.clone();
            for (k, v) in &args[1..] {
                let key = Expr::Str(vec![StrPart::Lit(k.clone().unwrap())]);
                out.retain(|(ok, _)| format!("{ok:?}") != format!("{key:?}"));
                out.push((key, v.clone()));
            }
            *e = Expr::Map(out);
        }
    });
}

fn lambdas_to_defs(body: &mut Expr, kids: &mut Vec<Item>, line: usize, skip: &[String]) {
    let whole = body.clone();
    lambdas_in_block(body, &whole, kids, line, skip);
}

// lambdas_to_defs for one block and the loop and if bodies inside it (`for l in lines: ok = lambda q: ...`),
// counting a name's uses over the whole body.
fn lambdas_in_block(body: &mut Expr, whole: &Expr, kids: &mut Vec<Item>, line: usize, skip: &[String]) {
    let Expr::Block(stmts, tail) = body else { return };
    for st in stmts.iter_mut() {
        if let Stmt::Expr { expr, .. } = st {
            match expr {
                Expr::For(_, _, b) | Expr::While(_, b) => lambdas_in_block(b, whole, kids, line, skip),
                Expr::If(_, t, f) => {
                    lambdas_in_block(t, whole, kids, line, skip);
                    if let Some(f) = f {
                        lambdas_in_block(f, whole, kids, line, skip);
                    }
                }
                _ => {}
            }
        }
    }
    let mut i = 0;
    while i < stmts.len() {
        // `up = (lambda s: s.upper()) if by == 'w' else (lambda s: s)`: the condition is kept in `up_cond`
        // and `up` is one lambda that picks its body by it.
        if let Stmt::Assign { target: Expr::Ident(n), op: "=", value: Expr::If(c, t, Some(f)), line: l } = &stmts[i] {
            let bare = |x: &Expr| match x {
                Expr::Block(st, Some(v)) if st.is_empty() => v.as_ref().clone(),
                x => x.clone(),
            };
            if let (Expr::Lambda(pa, ba), Expr::Lambda(pb, bb)) = (bare(t), bare(f)) {
                if pa == pb {
                    let (n, l) = (n.clone(), *l);
                    let flag = format!("{n}_cond");
                    let cond = Stmt::Assign { target: Expr::Ident(flag.clone()), op: "=", value: c.as_ref().clone(), line: l };
                    let body = Expr::If(Box::new(Expr::Ident(flag)), Box::new(Expr::Block(vec![], Some(ba))), Some(Box::new(Expr::Block(vec![], Some(bb)))));
                    stmts[i] = Stmt::Assign { target: Expr::Ident(n), op: "=", value: Expr::Lambda(pa, Box::new(body)), line: l };
                    stmts.insert(i, cond);
                    i += 1;
                }
            }
        }
        let Stmt::Assign { target: Expr::Ident(n), op: "=", value: Expr::Lambda(ps, lb), line: l } = &stmts[i] else {
            i += 1;
            continue;
        };
        let n = n.clone();
        // Set once, and never read as a value (`key=bad`, `f = bad`) or named by a nested def already.
        let mut sets = 0;
        let mut idents = 0;
        let _ = &tail;
        let mut whole = whole.clone();
        walk_mut(&mut whole, &mut |e| match e {
            Expr::Ident(x) | Expr::Move(x) if *x == n => idents += 1,
            Expr::Block(ss, _) => {
                for s in ss.iter() {
                    match s {
                        Stmt::Assign { target: Expr::Ident(x), .. } if *x == n => sets += 1,
                        Stmt::Let { name: Pat::Bind(x), .. } if *x == n => sets += 2,
                        _ => {}
                    }
                }
            }
            _ => {}
        });
        let as_value = idents > sets;
        let clash = kids.iter().any(|k| matches!(k, Item::Fn { name, .. } if name == &n));
        if sets != 1 || as_value || clash || skip.contains(&n) || ps.iter().any(|p| p == &n) {
            i += 1;
            continue;
        }
        let l = *l;
        let lb = lb.as_ref().clone();
        // A lambda whose body changes something (`out.append(x)`, `print(x)`) gives None.
        let none = match &lb {
            Expr::Method(_, m, _, _) => ["push", "append", "extend", "insert", "remove", "add", "clear", "sort", "update", "discard", "reverse"].contains(&m.as_str()),
            Expr::Call(f, _, _) => f == "print",
            _ => false,
        };
        let fb = if none { Expr::Block(vec![Stmt::Expr { expr: lb, line: l }], None) } else { Expr::Block(vec![], Some(Box::new(lb))) };
        let params = ps.iter().map(|p| Param { name: p.clone(), ty: Ty::Named("?".into()), is_mut: false }).collect();
        kids.push(Item::Fn { name: n, params, ret: Ty::Named("?".into()), fallible: false, body: fb, line: l.max(line), script: false });
        stmts.remove(i);
    }
}

// Python's nested `def inner(a):` inside `def outer(...)` becomes the top-level `outer_inner(a, c1, c2)`,
// where c1, c2 are the outer function's locals it reads; calls inside outer pass them along.
fn lift_nested(outer: &str, outer_params: &[Param], body: &mut Expr, kids: Vec<Item>) -> Vec<Item> {
    let mut outer_locals: Vec<String> = outer_params.iter().map(|p| p.name.clone()).collect();
    walk_mut(body, &mut |e| {
        if let Expr::Block(stmts, _) = e {
            for st in stmts.iter() {
                match st {
                    Stmt::Assign { target, .. } => pat_names_expr(target, &mut outer_locals),
                    Stmt::Let { name, .. } => pat_names(name, &mut outer_locals),
                    _ => {}
                }
            }
        }
        if let Expr::For(p, _, _) = e {
            pat_names(p, &mut outer_locals);
        }
    });
    let names: Vec<String> = kids.iter().filter_map(|k| match k { Item::Fn { name, .. } => Some(name.clone()), _ => None }).collect();
    // What each child reads from the outer function, and which of those it sets (`nonlocal`).
    let mut caps: Vec<Vec<String>> = Vec::new();
    let mut nonlocals: Vec<Vec<String>> = Vec::new();
    let mut owns: Vec<Vec<String>> = Vec::new();
    let mut sib_calls: Vec<Vec<usize>> = Vec::new();
    for k in &kids {
        let Item::Fn { params, body: kb, .. } = k else {
            caps.push(vec![]);
            nonlocals.push(vec![]);
            owns.push(vec![]);
            sib_calls.push(vec![]);
            continue;
        };
        let mut own: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        let mut kb2 = kb.clone();
        walk_mut(&mut kb2, &mut |e| {
            if let Expr::Block(stmts, _) = e {
                for st in stmts.iter() {
                    if let Stmt::Assign { target, .. } = st {
                        pat_names_expr(target, &mut own);
                    }
                }
            }
            if let Expr::For(p, _, _) = e {
                pat_names(p, &mut own);
            }
            if let Expr::Lambda(ps, _) = e {
                own.extend(ps.iter().cloned());
            }
        });
        // `nonlocal x` names the outer function's x: it is passed in, to be changed there.
        let mut shared: Vec<String> = Vec::new();
        walk_mut(&mut kb2, &mut |e| {
            if let Expr::Call(f, args, _) = e {
                if f == "__global" {
                    for (_, a) in args.iter() {
                        if let Expr::Ident(n) = a {
                            if outer_locals.contains(n) && !shared.contains(n) {
                                shared.push(n.clone());
                            }
                        }
                    }
                }
            }
        });
        own.retain(|n| !shared.contains(n));
        let mut used = Vec::new();
        walk_mut(&mut kb2, &mut |e| {
            if let Expr::Ident(n) | Expr::Move(n) = e {
                if outer_locals.contains(n) && !own.contains(n) && !used.contains(n) {
                    used.push(n.clone());
                }
            }
        });
        // A nonlocal name the child never reads is not passed.
        shared.retain(|n| used.contains(n));
        let mut calls = Vec::new();
        walk_mut(&mut kb2, &mut |e| {
            if let Expr::Call(f, _, _) = e {
                if let Some(j) = names.iter().position(|n| n == f) {
                    if !calls.contains(&j) {
                        calls.push(j);
                    }
                }
            }
        });
        caps.push(used);
        nonlocals.push(shared);
        owns.push(own);
        sib_calls.push(calls);
    }
    // A child that calls a sibling (`expr()` calling `peek()`) passes on what the sibling reads, so it
    // reads those names too.
    loop {
        let mut grew = false;
        for i in 0..caps.len() {
            for &j in &sib_calls[i].clone() {
                for c in caps[j].clone() {
                    if !owns[i].contains(&c) && !caps[i].contains(&c) {
                        caps[i].push(c.clone());
                        grew = true;
                    }
                    if nonlocals[j].contains(&c) && !nonlocals[i].contains(&c) && !owns[i].contains(&c) {
                        nonlocals[i].push(c);
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    let mangled = |n: &str| format!("{outer}_{n}");
    let rename = |e: &mut Expr| {
        walk_mut(e, &mut |x| {
            match x {
                Expr::Call(f, args, line) => {
                    if let Some(i) = names.iter().position(|n| n == f) {
                        *f = mangled(&names[i]);
                        for c in &caps[i] {
                            args.push((None, Expr::Ident(c.clone())));
                        }
                        let _ = line;
                    }
                }
                Expr::Ident(n) => {
                    if let Some(i) = names.iter().position(|m| m == n) {
                        if caps[i].is_empty() {
                            *n = mangled(&names[i]);
                        }
                    }
                }
                _ => {}
            }
        });
    };
    rename(body);
    let mut out = Vec::new();
    for ((k, cap), shared) in kids.into_iter().zip(caps.iter()).zip(nonlocals.iter()) {
        if let Item::Fn { name, mut params, ret, fallible, mut body, line, script } = k {
            for c in cap {
                params.push(Param { name: c.clone(), ty: Ty::Named("?".into()), is_mut: shared.contains(c) });
            }
            // The nonlocal names are parameters now, not top-level names.
            walk_mut(&mut body, &mut |e| {
                if let Expr::Block(st, _) = e {
                    for s in st.iter_mut() {
                        if let Stmt::Expr { expr: Expr::Call(f, args, _), .. } = s {
                            if f == "__global" {
                                args.retain(|(_, a)| !matches!(a, Expr::Ident(n) if shared.contains(n)));
                            }
                        }
                    }
                    st.retain(|s| !matches!(s, Stmt::Expr { expr: Expr::Call(f, args, _), .. } if f == "__global" && args.is_empty()));
                }
            });
            rename(&mut body);
            out.push(Item::Fn { name: mangled(&name), params, ret, fallible, body, line, script });
        } else {
            out.push(k);
        }
    }
    out
}

pub fn pat_names(p: &Pat, out: &mut Vec<String>) {
    match p {
        Pat::Bind(n) => {
            if !out.contains(n) {
                out.push(n.clone())
            }
        }
        Pat::Tuple(ps) | Pat::Variant(_, ps) => ps.iter().for_each(|p| pat_names(p, out)),
        _ => {}
    }
}

fn pat_names_expr(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Ident(n) => {
            if !out.contains(n) {
                out.push(n.clone())
            }
        }
        Expr::Tuple(xs) => xs.iter().for_each(|x| pat_names_expr(x, out)),
        _ => {}
    }
}

// Calls f on every expression inside e (children first is not needed; f sees each node once).
// A generator `def f(): ... yield x ...` runs to the end and returns the list of what it yielded,
// so `for v in f()`, `list(f())` and `sum(f())` work. (It is not lazy: an endless generator never returns.)
fn generator_body(body: Expr, line: usize) -> Expr {
    let acc = || Expr::Ident("ax_gen".into());
    let (mut stmts, tail) = match body {
        Expr::Block(st, tail) => (st, tail),
        e => (vec![], Some(Box::new(e))),
    };
    if let Some(t) = tail {
        stmts.push(Stmt::Expr { expr: *t, line });
    }
    let mut b = Expr::Block(stmts, None);
    walk_mut(&mut b, &mut |x| {
        if let Expr::Block(st, _) = x {
            for s in st.iter_mut() {
                if let Stmt::Return { value: v @ None, .. } = s {
                    *v = Some(acc());
                }
            }
        }
    });
    let Expr::Block(mut stmts, _) = b else { unreachable!() };
    stmts.insert(0, Stmt::Assign { target: acc(), op: "=", value: Expr::List(vec![]), line });
    stmts.push(Stmt::Return { value: Some(acc()), line });
    Expr::Block(stmts, None)
}

// Python code reads a tuple of like items as a sequence (`for d in ("Mon", "Tue"):`, `DAYS = ("Mon", "Tue")`
// then `for d in DAYS`, `len(DAYS)`). Such tuples are lists here: a tuple literal looped over directly, and an
// ALL_CAPS constant set to a tuple of like literals.
fn tuples_as_lists(items: &mut [Item], script: &mut [Stmt]) {
    fn kind(e: &Expr) -> Option<String> {
        match e {
            Expr::Int(_) | Expr::Unary("-", _) => Some("int".into()),
            Expr::Float(_) => Some("float".into()),
            Expr::Str(p) if p.iter().all(|x| matches!(x, StrPart::Lit(_))) => Some("str".into()),
            Expr::Tuple(xs) => Some(format!("({})", xs.iter().map(|x| kind(x).unwrap_or("?".into())).collect::<Vec<_>>().join(","))),
            _ => None,
        }
    }
    let like = |xs: &[Expr]| xs.len() >= 2 && kind(&xs[0]).is_some_and(|k| !k.contains('?') && xs.iter().all(|x| kind(x).as_ref() == Some(&k)));
    let caps = |n: &str| n.chars().any(|c| c.is_ascii_uppercase()) && n.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    let fix = |e: &mut Expr| {
        walk_mut(e, &mut |x| {
            if let Expr::For(_, it, _) = x {
                if let Expr::Tuple(xs) = it.as_mut() {
                    *it = Box::new(Expr::List(std::mem::take(xs)));
                }
            }
            if let Expr::Block(st, _) = x {
                for s in st.iter_mut() {
                    if let Stmt::Assign { target: Expr::Ident(n), op: "=", value, .. } | Stmt::Let { name: Pat::Bind(n), value, .. } = s {
                        if caps(n) {
                            if let Expr::Tuple(xs) = value {
                                if like(xs) {
                                    *value = Expr::List(std::mem::take(xs));
                                }
                            }
                        }
                    }
                }
            }
        });
    };
    for it in items.iter_mut() {
        if let Item::Fn { body, .. } = it {
            fix(body);
        }
    }
    let mut b = Expr::Block(script.to_vec(), None);
    fix(&mut b);
    if let Expr::Block(v, _) = b {
        for (st, n) in script.iter_mut().zip(v) {
            *st = n;
        }
    }
}

// Files opened in a block (`f = open(p)`, or from `with open(p) as f:`). Read mode: f holds the file's text, so
// `f.read()` is f, `f.readlines()` and `for line in f` give its lines with their "\n", and `f.close()` does nothing.
// Write and append mode: f is a list of the pieces written (`f.write(s)`, `print(..., file=f)`), and `f.close()`
// writes them out (append mode adds them after the file's text, if the file is there).
fn fix_opens(stmts: &mut [Stmt]) {
    for i in 0..stmts.len() {
        let (name, kind, path, line) = match &stmts[i] {
            Stmt::Let { name: Pat::Bind(n), value: Expr::Call(k, a, l), .. } | Stmt::Assign { target: Expr::Ident(n), op: "=", value: Expr::Call(k, a, l), .. }
                if k.starts_with("__open_") && a.len() == 1 =>
            {
                (n.clone(), k.clone(), a[0].1.clone(), *l)
            }
            _ => continue,
        };
        let id = || Expr::Ident(name.clone());
        let is_f = |e: &Expr| matches!(e, Expr::Ident(n) if *n == name);
        let read = kind == "__open_r";
        let value = if read { Expr::Call("read".into(), vec![(None, path.clone())], line) } else { Expr::List(vec![]) };
        match &mut stmts[i] {
            Stmt::Let { value: v, mutable, .. } => {
                *v = value;
                *mutable = true;
            }
            Stmt::Assign { value: v, .. } => *v = value,
            _ => {}
        }
        // Written out at close: the pieces joined, after the old text in append mode.
        let joined = Expr::Method(Box::new(id()), "join".into(), vec![Expr::Str(vec![])], line);
        let text = if kind == "__open_a" {
            let old = Expr::Call("__try_read".into(), vec![(None, path.clone())], line);
            Expr::Binary("+", Box::new(old), Box::new(joined))
        } else {
            joined
        };
        let save = Expr::Call("write".into(), vec![(None, path.clone()), (None, text)], line);
        let mut fix = |e: &mut Expr| {
            let new = match e {
                Expr::Method(r, m, a, l) if is_f(r) => match (m.as_str(), a.len(), read) {
                    ("read", 0, true) => Some(id()),
                    ("readlines", 0, true) => Some(Expr::Method(Box::new(id()), "lines_keep".into(), vec![], *l)),
                    ("close", 0, true) => Some(Expr::Block(vec![], None)),
                    ("close", 0, false) => Some(save.clone()),
                    ("write", 1, false) => Some(Expr::Method(Box::new(id()), "push".into(), vec![a[0].clone()], *l)),
                    ("flush", 0, _) => Some(Expr::Block(vec![], None)),
                    _ => None,
                },
                Expr::For(_, it, _) if read && is_f(it) => {
                    **it = Expr::Method(Box::new(id()), "lines_keep".into(), vec![], line);
                    None
                }
                Expr::Call(p, a, l) if p == "print" && !read && a.iter().any(|(n, v)| n.as_deref() == Some("file") && is_f(v)) => {
                    let end = a.iter().find(|(n, _)| n.as_deref() == Some("end")).map(|(_, v)| v.clone()).unwrap_or(Expr::Str(vec![StrPart::Lit("\n".into())]));
                    let sep = a.iter().find(|(n, _)| n.as_deref() == Some("sep")).map(|(_, v)| v.clone()).unwrap_or(Expr::Str(vec![StrPart::Lit(" ".into())]));
                    let items: Vec<Expr> = a.iter().filter(|(n, _)| n.is_none()).map(|(_, v)| Expr::Call("str".into(), vec![(None, v.clone())], *l)).collect();
                    let body = Expr::Method(Box::new(Expr::List(items)), "join".into(), vec![sep], *l);
                    Some(Expr::Method(Box::new(id()), "push".into(), vec![Expr::Binary("+", Box::new(body), Box::new(end))], *l))
                }
                _ => None,
            };
            if let Some(n) = new {
                *e = n;
            }
        };
        for st in stmts[i + 1..].iter_mut() {
            let mut b = Expr::Block(vec![st.clone()], None);
            walk_mut(&mut b, &mut fix);
            if let Expr::Block(mut v, _) = b {
                *st = v.remove(0);
            }
            // Stop at the next `f = ...`: f is something else from there on.
            if matches!(st, Stmt::Let { name: Pat::Bind(n), .. } | Stmt::Assign { target: Expr::Ident(n), op: "=", .. } if *n == name) {
                break;
            }
        }
    }
}

pub(crate) fn walk_mut(e: &mut Expr, f: &mut dyn FnMut(&mut Expr)) {
    f(e);
    match e {
        Expr::List(xs) | Expr::Tuple(xs) => xs.iter_mut().for_each(|x| walk_mut(x, f)),
        Expr::Map(ps) => ps.iter_mut().for_each(|(k, v)| {
            walk_mut(k, f);
            walk_mut(v, f);
        }),
        Expr::Str(parts) => {
            for p in parts.iter_mut() {
                if let StrPart::Interp(x) = p {
                    walk_mut(x, f);
                }
            }
        }
        Expr::Unary(_, a) | Expr::Field(a, _) | Expr::TupleField(a, _) | Expr::Lambda(_, a) | Expr::Try(a) => walk_mut(a, f),
        Expr::Binary(_, a, b) | Expr::Index(a, b) | Expr::Range(a, b) | Expr::While(a, b) | Expr::For(_, a, b) => {
            walk_mut(a, f);
            walk_mut(b, f);
        }
        Expr::Call(_, args, _) => args.iter_mut().for_each(|(_, a)| walk_mut(a, f)),
        Expr::Method(r, _, args, _) => {
            walk_mut(r, f);
            args.iter_mut().for_each(|a| walk_mut(a, f));
        }
        Expr::If(c, a, b) => {
            walk_mut(c, f);
            walk_mut(a, f);
            if let Some(b) = b {
                walk_mut(b, f);
            }
        }
        Expr::Match(sub, arms) => {
            walk_mut(sub, f);
            arms.iter_mut().for_each(|(_, a)| walk_mut(a, f));
        }
        Expr::Block(stmts, tail) => {
            for st in stmts.iter_mut() {
                match st {
                    Stmt::Let { value, .. } => walk_mut(value, f),
                    Stmt::Assign { target, value, .. } => {
                        walk_mut(target, f);
                        walk_mut(value, f);
                    }
                    Stmt::Expr { expr, .. } | Stmt::Return { value: Some(expr), .. } | Stmt::Assert { cond: expr, .. } => walk_mut(expr, f),
                    _ => {}
                }
            }
            if let Some(t) = tail {
                walk_mut(t, f);
            }
        }
        _ => {}
    }
}

// Calls on objects of the program's classes: `C(a)` is `C_new(a)` and `obj.m(x)` is `C_m(obj, x)`. A method name
// that a built-in method also has is only taken for `self.m(x)` inside the class, or when it is no built-in.
fn lower_methods(items: &mut [Item], script: &mut [Stmt], classes: &[(Item, Vec<String>)]) {
    let fn_names: Vec<String> = items.iter().filter_map(|i| match i { Item::Fn { name, .. } => Some(name.clone()), _ => None }).collect();
    let class_names: Vec<String> = classes.iter().filter_map(|(c, _)| match c { Item::Type { name, .. } => Some(name.clone()), _ => None }).collect();
    // Classes whose fields all come from methods: `C()` starts from the zero value of each.
    let plain: Vec<String> = classes
        .iter()
        .filter_map(|(c, _)| match c {
            Item::Type { name, fields, .. } if fields.iter().all(|f| matches!(&f.ty, Ty::Named(n) if n == "?")) => Some(name.clone()),
            _ => None,
        })
        .collect();
    let owners = |m: &str| -> Vec<String> { class_names.iter().filter(|c| fn_names.contains(&format!("{c}_{m}"))).cloned().collect() };
    let lower = |e: &mut Expr, me: Option<&str>| {
        // Names set from a constructor (`ps = Parser(toks)`) are known to hold that class.
        let mut held: Vec<(String, String)> = Vec::new();
        walk_mut(e, &mut |x| {
            if let Expr::Block(stmts, _) = x {
                for st in stmts.iter() {
                    if let Stmt::Assign { target: Expr::Ident(n), op: "=", value: Expr::Call(c, _, _), .. } | Stmt::Let { name: Pat::Bind(n), value: Expr::Call(c, _, _), .. } = st {
                        if class_names.contains(c) {
                            held.push((n.clone(), c.clone()));
                        }
                    }
                }
            }
        });
        walk_mut(e, &mut |x| match x {
            Expr::Call(f, _, _) if class_names.contains(f) && fn_names.contains(&format!("{f}_new")) => *f = format!("{f}_new"),
            Expr::Call(f, args, _) if class_names.contains(f) && args.is_empty() && plain.contains(f) => *f = "__default".into(),
            Expr::Method(r, m, args, line) => {
                let os = owners(m);
                let pick = match (me, r.as_ref()) {
                    (Some(c), Expr::Ident(n)) if n == "self_" && os.iter().any(|o| o == c) => Some(c.to_string()),
                    (_, Expr::Ident(n)) if held.iter().any(|(h, c)| h == n && os.contains(c)) => held.iter().find(|(h, _)| h == n).map(|(_, c)| c.clone()),
                    // `Bird("x").describe()`: the receiver is a fresh object.
                    (_, Expr::Call(f, _, _)) if os.iter().any(|o| f == o || *f == format!("{o}_new")) => {
                        os.iter().find(|o| f == *o || *f == format!("{o}_new")).cloned()
                    }
                    _ if os.len() == 1 && !crate::emit::METHODS.contains(&m.as_str()) && !PY_METHODS.contains(&m.as_str()) => Some(os[0].clone()),
                    _ => None,
                };
                if let Some(c) = pick {
                    let mut a = vec![(None, *r.clone())];
                    a.extend(args.drain(..).map(|x| (None, x)));
                    *x = Expr::Call(format!("{c}_{m}"), a, *line);
                }
            }
            _ => {}
        });
    };
    for it in items.iter_mut() {
        if let Item::Fn { name, body, .. } = it {
            let me = class_names.iter().find(|c| name.starts_with(&format!("{c}_"))).cloned();
            lower(body, me.as_deref());
        }
    }
    let mut b = Expr::Block(script.to_vec(), None);
    lower(&mut b, None);
    if let Expr::Block(v, _) = b {
        for (st, n) in script.iter_mut().zip(v) {
            *st = n;
        }
    }
}

// `class B(A):` gets A's fields and the methods it does not define itself, copied with `self` as a B.
// `super().__init__(x)` in B's __init__ runs A's __init__ body there.
fn inherit(items: &mut Vec<Item>, classes: &mut [(Item, Vec<String>)]) {
    let names: Vec<String> = classes.iter().filter_map(|(c, _)| match c { Item::Type { name, .. } => Some(name.clone()), _ => None }).collect();
    for ci in 0..classes.len() {
        let Some(base) = classes[ci].1.iter().find(|b| names.contains(b)).cloned() else { continue };
        let Item::Type { name: me, .. } = &classes[ci].0 else { continue };
        let me = me.clone();
        let base_fields = classes.iter().find_map(|(c, _)| match c {
            Item::Type { name, fields, .. } if *name == base => Some(fields.clone()),
            _ => None,
        });
        if let (Some(bf), Item::Type { fields, .. }) = (base_fields, &mut classes[ci].0) {
            for f in bf {
                if !fields.iter().any(|g| g.name == f.name) {
                    fields.push(f);
                }
            }
        }
        let prefix = format!("{base}_");
        let have: Vec<String> = items.iter().filter_map(|i| match i { Item::Fn { name, .. } => Some(name.clone()), _ => None }).collect();
        let base_new = items.iter().find_map(|i| match i {
            Item::Fn { name, params, body, .. } if *name == format!("{base}_new") => Some((params.clone(), body.clone())),
            _ => None,
        });
        let mut copies = Vec::new();
        for it in items.iter() {
            let Item::Fn { name, params, ret, fallible, body, line, script } = it else { continue };
            let Some(m) = name.strip_prefix(&prefix) else { continue };
            let new_name = format!("{me}_{m}");
            if have.contains(&new_name) {
                continue;
            }
            let mut params = params.clone();
            let mut body = body.clone();
            if m == "new" {
                if let Expr::Block(st, _) = &mut body {
                    if let Some(Stmt::Let { ty, .. }) = st.first_mut() {
                        *ty = Some(Ty::Named(me.clone()));
                    }
                }
            } else if let Some(p) = params.first_mut() {
                if p.name == "self_" {
                    p.ty = Ty::Named(me.clone());
                }
            }
            copies.push(Item::Fn { name: new_name, params, ret: ret.clone(), fallible: *fallible, body, line: *line, script: *script });
        }
        items.extend(copies);
        // super().__init__(args) inside our own __init__.
        if let Some((bps, Expr::Block(bst, _))) = base_new {
            for it in items.iter_mut() {
                let Item::Fn { name, body: Expr::Block(st, _), .. } = it else { continue };
                if *name != format!("{me}_new") {
                    continue;
                }
                let mut out = Vec::new();
                for s in st.drain(..) {
                    match &s {
                        Stmt::Expr { expr: Expr::Method(r, m, args, _), line } if m == "__init__" && matches!(r.as_ref(), Expr::Call(f, _, _) if f == "super_") => {
                            for (p, a) in bps.iter().zip(args.iter()) {
                                out.push(Stmt::Let { name: Pat::Bind(p.name.clone()), ty: None, value: a.clone(), mutable: true, line: *line });
                            }
                            out.extend(bst.iter().skip(1).cloned());
                        }
                        _ => out.push(s),
                    }
                }
                *st = out;
            }
        }
    }
}

// Calls to functions with default values or `*args`: keyword arguments go to their places, left-out
// ones take their default, and the arguments for `*args` become one list.
fn fill_args(items: &mut [Item], script: &mut [Stmt], sigs: &HashMap<String, Sig>) {
    let fix = |e: &mut Expr| {
        walk_mut(e, &mut |x| {
            let Expr::Call(f, args, _) = x else { return };
            let Some((names, defaults, star)) = sigs.get(f.as_str()) else { return };
            let n = names.len();
            let mut slots: Vec<Option<Expr>> = vec![None; n];
            let mut rest = Vec::new();
            let mut k = 0;
            for (name, a) in args.drain(..) {
                match name {
                    Some(nm) => match names.iter().position(|p| *p == nm) {
                        Some(i) => slots[i] = Some(a),
                        None => rest.push(a),
                    },
                    None => {
                        if Some(k) == *star {
                            rest.push(a);
                            continue;
                        }
                        if k < n {
                            slots[k] = Some(a);
                        }
                        k += 1;
                    }
                }
            }
            if let Some(si) = star {
                slots[*si] = Some(Expr::List(rest));
            }
            let mut out = Vec::new();
            for (i, s) in slots.into_iter().enumerate() {
                match s.or_else(|| defaults[i].clone()) {
                    Some(v) => out.push((None, v)),
                    None => break,
                }
            }
            *args = out;
        });
    };
    for it in items.iter_mut() {
        if let Item::Fn { body, .. } = it {
            fix(body);
        }
    }
    let mut b = Expr::Block(script.to_vec(), None);
    fix(&mut b);
    if let Expr::Block(v, _) = b {
        for (st, n) in script.iter_mut().zip(v) {
            *st = n;
        }
    }
}

// Python's own method names: on a value not known to be one of the program's classes, they stay built-in.
const PY_METHODS: &[&str] = &[
    "add", "extend", "append", "pop", "get", "update", "remove", "discard", "clear", "copy", "index", "count", "keys", "values",
    "items", "split", "strip", "replace", "find", "insert", "setdefault", "popleft", "appendleft", "most_common", "match", "search",
    "sub", "group", "groups", "findall", "finditer", "start", "end", "span", "fullmatch", "lower", "upper", "startswith", "endswith", "format", "read", "write",
];

// An expression that is plainly a bool already (a comparison, `not`, `and`/`or`, a test method).
fn is_bool_expr(e: &Expr) -> bool {
    match e {
        Expr::Bool(_) => true,
        Expr::Unary("!", _) => true,
        Expr::Binary(op, _, _) => matches!(*op, "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||" | "in" | "not in"),
        Expr::Method(_, m, _, _) => matches!(m.as_str(), "is_none" | "is_empty" | "contains" | "has" | "starts_with" | "ends_with" | "is_digit" | "is_alpha" | "is_space" | "is_upper" | "is_lower" | "is_alnum" | "any" | "all" | "isinstance"),
        _ => false,
    }
}

// `out = print`, then `out(...)`: each call is a print (the name is set to 0, which nothing reads).
fn print_aliases(toks: &mut [crate::lexer::Token]) {
    let mut names = vec![];
    for i in 0..toks.len().saturating_sub(3) {
        if let (Tok::Ident(x), Tok::Sym("="), Tok::Ident(p), Tok::Newline) = (&toks[i].tok, &toks[i + 1].tok, &toks[i + 2].tok, &toks[i + 3].tok) {
            if p == "print" && (i == 0 || matches!(toks[i - 1].tok, Tok::Newline | Tok::Sym("{") | Tok::Sym("}") | Tok::Sym(";"))) {
                names.push((x.clone(), i));
            }
        }
    }
    // Only a name set nowhere else.
    names.retain(|(x, at)| !toks.windows(2).enumerate().any(|(j, w)| j != *at && matches!((&w[0].tok, &w[1].tok), (Tok::Ident(y), Tok::Sym("=")) if y == x)));
    for (x, at) in names {
        toks[at + 2].tok = Tok::Int(0);
        for j in 0..toks.len() - 1 {
            if j != at && matches!(&toks[j].tok, Tok::Ident(y) if *y == x) && matches!(toks[j + 1].tok, Tok::Sym("(")) {
                toks[j].tok = Tok::Ident("print".into());
            }
        }
    }
}

// `type(v)` on one side and a type name on the other.
fn type_test(a: &Expr, b: &Expr) -> Option<(Expr, String)> {
    let Expr::Call(f, args, _) = a else { return None };
    if f != "ax_type_of" || args.len() != 1 {
        return None;
    }
    let name = match b {
        Expr::Ident(n) => n.clone(),
        Expr::Lambda(_, body) => match body.as_ref() {
            Expr::Method(_, n, a, _) if a.is_empty() => n.clone(),
            _ => return None,
        },
        _ => return None,
    };
    let name = match name.as_str() {
        "set" | "List" | "tuple" => "list".to_string(),
        "Dict" | "OrderedDict" | "defaultdict" => "dict".to_string(),
        _ => name,
    };
    Some((args[0].1.clone(), name))
}

// `json.dumps(x, ...)` becomes `x.py_dumps(item_sep, key_sep, indent, sort_keys, ensure_ascii)`.
// indent -1 is none; Python's default separators are ", " and ": ", or "," and ": " with an indent.
fn py_dumps(mut raw: Vec<(Option<String>, Expr)>, line: usize) -> Expr {
    let x = raw.remove(0).1;
    let lit = |s: &str| Expr::Str(vec![StrPart::Lit(s.to_string())]);
    let mut indent = Expr::Unary("-", Box::new(Expr::Int(1)));
    let mut seps: Option<(Expr, Expr)> = None;
    let mut sort = Expr::Bool(false);
    let mut ascii = Expr::Bool(true);
    for (k, v) in raw {
        match k.as_deref() {
            Some("indent") => {
                if !matches!(&v, Expr::Ident(n) if n == "None") && !matches!(&v, Expr::Call(n, a, _) if n == "none" && a.is_empty()) {
                    indent = v;
                }
            }
            Some("separators") => {
                if let Expr::Tuple(mut xs) = v {
                    if xs.len() == 2 {
                        let b = xs.pop().unwrap();
                        seps = Some((xs.pop().unwrap(), b));
                    }
                }
            }
            Some("sort_keys") => sort = v,
            Some("ensure_ascii") => ascii = v,
            _ => {}
        }
    }
    let has_indent = !matches!(&indent, Expr::Unary("-", _));
    let (item, key) = seps.unwrap_or_else(|| (lit(if has_indent { "," } else { ", " }), lit(": ")));
    Expr::Method(Box::new(x), "py_dumps".into(), vec![item, key, indent, sort, ascii], line)
}

// Python's `operator.lt`, `operator.add` and the like as a lambda of two arguments.
fn operator_fn(name: &str, line: usize) -> Option<Expr> {
    let op: &'static str = match name {
        "lt" => "<",
        "le" => "<=",
        "gt" => ">",
        "ge" => ">=",
        "eq" => "==",
        "ne" => "!=",
        "add" => "+",
        "sub" => "-",
        "mul" => "*",
        "mod" => "%",
        "floordiv" => "//",
        "truediv" => "/",
        _ => return None,
    };
    let (a, b) = (Box::new(Expr::Ident("ax_oa".into())), Box::new(Expr::Ident("ax_ob".into())));
    let body = if op == "/" { Expr::Call("__truediv".into(), vec![(None, *a), (None, *b)], line) } else { Expr::Binary(op, a, b) };
    Some(Expr::Lambda(vec!["ax_oa".into(), "ax_ob".into()], Box::new(body)))
}

// Python's `CMP = {'<': operator.lt, '>': lambda a, b: a > b}` set once at the top level and only called
// as `CMP[op](a, b)`: each call picks the function by its key in place (a missing key is a KeyError),
// so no map of functions is needed.
fn inline_fn_maps(items: &mut [Item], script: &mut Vec<Stmt>) {
    let mut i = 0;
    while i < script.len() {
        let (name, ps) = match &script[i] {
            Stmt::Assign { target: Expr::Ident(n), op: "=", value: Expr::Map(ps), .. } | Stmt::Let { name: Pat::Bind(n), value: Expr::Map(ps), .. } => (n.clone(), ps.clone()),
            _ => {
                i += 1;
                continue;
            }
        };
        let mut arms: Vec<(String, Vec<String>, Expr)> = vec![];
        for (k, v) in &ps {
            match (k, v) {
                (Expr::Str(parts), Expr::Lambda(params, body)) if matches!(parts.as_slice(), [StrPart::Lit(_)]) => {
                    let [StrPart::Lit(k)] = parts.as_slice() else { unreachable!() };
                    arms.push((k.clone(), params.clone(), (**body).clone()));
                }
                _ => break,
            }
        }
        let n_args = arms.first().map_or(0, |a| a.1.len());
        if arms.is_empty() || arms.len() != ps.len() || arms.iter().any(|a| a.1.len() != n_args || format!("{:?}", a.2).contains(&format!("Ident({name:?})"))) {
            i += 1;
            continue;
        }
        // Never set again or changed (nor a local of that name somewhere).
        let bad = [format!("target: Ident({name:?})"), format!("target: Index(Ident({name:?})"), format!("Method(Ident({name:?})"), format!("Bind({name:?})"), format!("Field(Ident({name:?})")];
        let texts: Vec<String> = items.iter().filter_map(|it| if let Item::Fn { body, .. } = it { Some(format!("{body:?}")) } else { None }).chain(script.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, st)| format!("{st:?}"))).collect();
        if texts.iter().any(|t| bad.iter().any(|b| t.contains(b.as_str()))) {
            i += 1;
            continue;
        }
        let call = |e: &mut Expr| {
            let Expr::Method(r, m, args, line) = e else { return };
            if m != "__call" || args.len() != n_args {
                return;
            }
            let Expr::Index(b, k) = r.as_ref() else { return };
            if !matches!(b.as_ref(), Expr::Ident(x) if *x == name) {
                return;
            }
            let line = *line;
            let tmp = |j: usize| format!("ax_fm{j}");
            let mut lets = vec![Stmt::Let { name: Pat::Bind("ax_fmk".into()), ty: None, value: (**k).clone(), mutable: false, line }];
            for (j, a) in args.iter().enumerate() {
                lets.push(Stmt::Let { name: Pat::Bind(tmp(j)), ty: None, value: a.clone(), mutable: false, line });
            }
            let missing = Expr::Call("fail".into(), vec![(None, Expr::Call("__exc".into(), vec![(None, Expr::Str(vec![StrPart::Lit("KeyError".into())])), (None, Expr::Str(vec![StrPart::Lit("'".into()), StrPart::Interp(Expr::Ident("ax_fmk".into())), StrPart::Lit("'".into())]))], line))], line);
            let mut chain = Expr::Block(vec![Stmt::Expr { expr: missing, line }], None);
            for (key, params, body) in arms.iter().rev() {
                let binds = params.iter().enumerate().map(|(j, p)| Stmt::Let { name: Pat::Bind(p.clone()), ty: None, value: Expr::Ident(tmp(j)), mutable: false, line }).collect();
                let cond = Expr::Binary("==", Box::new(Expr::Ident("ax_fmk".into())), Box::new(Expr::Str(vec![StrPart::Lit(key.clone())])));
                chain = Expr::If(Box::new(cond), Box::new(Expr::Block(binds, Some(Box::new(body.clone())))), Some(Box::new(chain)));
            }
            *e = Expr::Block(lets, Some(Box::new(chain)));
        };
        let mut call = call;
        for it in items.iter_mut() {
            if let Item::Fn { body, .. } = it {
                walk_mut(body, &mut call);
            }
        }
        for st in script.iter_mut() {
            match st {
                Stmt::Let { value: e, .. } | Stmt::Assign { value: e, .. } | Stmt::Expr { expr: e, .. } | Stmt::Return { value: Some(e), .. } => walk_mut(e, &mut call),
                _ => {}
            }
        }
        // Nothing else uses the map: it is left out.
        let used = items.iter().any(|it| matches!(it, Item::Fn { body, .. } if format!("{body:?}").contains(&format!("Ident({name:?})"))))
            || script.iter().enumerate().any(|(j, st)| j != i && format!("{st:?}").contains(&format!("Ident({name:?})")));
        if !used {
            script.remove(i);
            continue;
        }
        i += 1;
    }
}

// Python's None in a name that holds a list, dict, set or a tuple of them is kept as an empty one, so
// `cur is None` after `cur = ({}, set())` would wrongly be true. Such a name (set to None, set to an empty
// container, and tested with `is None`) gets a flag kept next to it: `ax_nn_cur` is true while it is None.
fn none_flags(items: &mut [Item], script: &mut Vec<Stmt>) {
    fn empty(e: &Expr) -> bool {
        match e {
            Expr::List(xs) => xs.is_empty(),
            Expr::Map(ps) => ps.is_empty(),
            Expr::Tuple(xs) => !xs.is_empty() && xs.iter().all(empty),
            _ => false,
        }
    }
    fn is_none(e: &Expr) -> bool {
        matches!(e, Expr::Call(n, a, _) if n == "none" && a.is_empty())
    }
    // Every setting of n in e: Some(value) for a plain `n = v`, None for any other kind (a loop name, a tuple).
    fn sets(e: &Expr, n: &str, out: &mut Vec<Option<Expr>>) {
        let mut e = e.clone();
        walk_mut(&mut e, &mut |x| {
            if let Expr::Block(st, _) = x {
                for s in st.iter() {
                    match s {
                        Stmt::Assign { target: Expr::Ident(m), op: "=", value, .. } | Stmt::Let { name: Pat::Bind(m), value, .. } if m == n => out.push(Some(value.clone())),
                        Stmt::Assign { target, .. } if format!("{target:?}") == format!("Ident({n:?})") => out.push(None),
                        Stmt::Assign { target: Expr::Tuple(xs), .. } if xs.iter().any(|x| matches!(x, Expr::Ident(m) if m == n)) => out.push(None),
                        Stmt::Let { name, .. } if !matches!(name, Pat::Bind(_)) && format!("{name:?}").contains(&format!("Bind({n:?})")) => out.push(None),
                        _ => {}
                    }
                }
            }
            if let Expr::For(p, _, _) = x {
                if format!("{p:?}").contains(&format!("Bind({n:?})")) {
                    out.push(None);
                }
            }
            if let Expr::Lambda(ps, _) = x {
                if ps.iter().any(|p| p == n) {
                    out.push(None);
                }
            }
        });
    }
    fn tested(e: &Expr, n: &str) -> bool {
        format!("{e:?}").contains(&format!("Method(Ident({n:?}), \"is_none\", [],"))
    }
    fn declares_global(e: &Expr, n: &str) -> bool {
        format!("{e:?}").contains(&format!("Call(\"__global\", [")) && {
            let mut hit = false;
            let mut e = e.clone();
            walk_mut(&mut e, &mut |x| {
                if let Expr::Call(f, args, _) = x {
                    if f == "__global" && args.iter().any(|(_, a)| matches!(a, Expr::Ident(m) if m == n)) {
                        hit = true;
                    }
                }
            });
            hit
        }
    }
    fn rewrite(e: &mut Expr, n: &str, flag: &str) {
        walk_mut(e, &mut |x| {
            match x {
                Expr::Method(r, m, a, _) if m == "is_none" && a.is_empty() && matches!(r.as_ref(), Expr::Ident(k) if k == n) => {
                    *x = Expr::Ident(flag.to_string());
                }
                Expr::Call(f, args, _) if f == "__global" && args.iter().any(|(_, a)| matches!(a, Expr::Ident(m) if m == n)) => {
                    args.push((None, Expr::Ident(flag.to_string())));
                }
                Expr::Block(st, _) => {
                    let mut i = 0;
                    while i < st.len() {
                        let add = match &st[i] {
                            Stmt::Assign { target: Expr::Ident(m), op: "=", value, line } if m == n => Some((is_none(value), *line, false)),
                            Stmt::Let { name: Pat::Bind(m), value, line, .. } if m == n => Some((is_none(value), *line, true)),
                            _ => None,
                        };
                        if let Some((none, line, is_let)) = add {
                            let v = Expr::Bool(none);
                            let s = if is_let { Stmt::Let { name: Pat::Bind(flag.to_string()), ty: None, value: v, mutable: true, line } } else { Stmt::Assign { target: Expr::Ident(flag.to_string()), op: "=", value: v, line } };
                            st.insert(i + 1, s);
                            i += 1;
                        }
                        i += 1;
                    }
                }
                _ => {}
            }
        });
    }
    // Names set at the top level (shared with functions that declare them global).
    let script_e = Expr::Block(script.clone(), None);
    let mut cands: Vec<String> = vec![];
    crate::infer::walk(&script_e, &mut |x| {
        if let Expr::Block(st, _) = x {
            for s in st {
                if let Stmt::Assign { target: Expr::Ident(m), op: "=", value, .. } | Stmt::Let { name: Pat::Bind(m), value, .. } = s {
                    if empty(value) && !cands.contains(m) {
                        cands.push(m.clone());
                    }
                }
            }
        }
    });
    for n in cands {
        let flag = format!("ax_nn_{n}");
        let mut all = vec![];
        sets(&script_e, &n, &mut all);
        let mut ok = true;
        let mut any_test = tested(&script_e, &n);
        for it in items.iter() {
            if let Item::Fn { params, body, .. } = it {
                let mut fs = vec![];
                sets(body, &n, &mut fs);
                let g = declares_global(body, &n);
                if params.iter().any(|p| p.name == n) || (!fs.is_empty() && !g) {
                    // Its own local of that name: tests there are not about the top-level one.
                    if tested(body, &n) {
                        ok = false;
                    }
                    continue;
                }
                any_test |= tested(body, &n);
                all.extend(fs);
            }
        }
        if !ok || !any_test || all.iter().any(|v| v.is_none()) || !all.iter().any(|v| v.as_ref().is_some_and(is_none)) {
            continue;
        }
        for it in items.iter_mut() {
            if let Item::Fn { params, body, .. } = it {
                if params.iter().any(|p| p.name == n) {
                    continue;
                }
                let mut fs = vec![];
                sets(body, &n, &mut fs);
                if !fs.is_empty() && !declares_global(body, &n) {
                    continue;
                }
                rewrite(body, &n, &flag);
            }
        }
        let mut b = Expr::Block(std::mem::take(script), None);
        rewrite(&mut b, &n, &flag);
        if let Expr::Block(st, _) = b {
            *script = st;
        }
    }
    // Names local to one function.
    for it in items.iter_mut() {
        let Item::Fn { params, body, .. } = it else { continue };
        let mut cands: Vec<String> = vec![];
        crate::infer::walk(body, &mut |x| {
            if let Expr::Block(st, _) = x {
                for s in st {
                    if let Stmt::Assign { target: Expr::Ident(m), op: "=", value, .. } | Stmt::Let { name: Pat::Bind(m), value, .. } = s {
                        if empty(value) && !cands.contains(m) {
                            cands.push(m.clone());
                        }
                    }
                }
            }
        });
        for n in cands {
            if params.iter().any(|p| p.name == n) || declares_global(body, &n) || !tested(body, &n) {
                continue;
            }
            let mut all = vec![];
            sets(body, &n, &mut all);
            if all.iter().any(|v| v.is_none()) || !all.iter().any(|v| v.as_ref().is_some_and(is_none)) {
                continue;
            }
            let flag = format!("ax_nn_{n}");
            rewrite(body, &n, &flag);
        }
    }
}

// Python's `def __init__(self, funcs, out=print)` with `self.out = out` and `self.out(text)`, where no call
// passes `out`: the parameter is left out and `self.out(text)` prints. (Likewise `def run(src, emit=print)`
// calling `emit(x)`.)
fn print_params(items: &mut [Item], script: &mut [Stmt], sigs: &mut HashMap<String, Sig>) {
    let names: Vec<String> = sigs.keys().cloned().collect();
    for f in names {
        let Some((pnames, defaults, star)) = sigs.get(&f).cloned() else { continue };
        let Some(i) = defaults.iter().position(|d| matches!(d, Some(Expr::Ident(n)) if n == "print")) else { continue };
        if star.is_some_and(|s| s <= i) {
            continue;
        }
        let p = pnames[i].clone();
        // No call passes it: count the arguments of every call to f (or to its class, for `C_new`).
        let class = f.strip_suffix("_new").map(String::from);
        let mut passes = false;
        let mut calls = 0;
        let mut check = |e: &Expr| {
            if let Expr::Call(c, args, _) = e {
                if *c == f || class.as_deref() == Some(c.as_str()) {
                    calls += 1;
                    let pos = args.iter().filter(|(n, _)| n.is_none()).count();
                    if pos > i || args.iter().any(|(n, _)| n.as_deref() == Some(p.as_str())) {
                        passes = true;
                    }
                }
            }
        };
        for it in items.iter() {
            if let Item::Fn { body, .. } = it {
                crate::infer::walk(body, &mut check);
            }
        }
        crate::infer::walk(&Expr::Block(script.to_vec(), None), &mut check);
        if passes || calls == 0 {
            continue;
        }
        let Some(fi) = items.iter().position(|it| matches!(it, Item::Fn { name, .. } if *name == f)) else { continue };
        // In a class: `self.F = p` once, and self.F only ever called.
        let mut field: Option<String> = None;
        if let Item::Fn { body, .. } = &items[fi] {
            crate::infer::walk(body, &mut |e| {
                if let Expr::Block(st, _) = e {
                    for s in st {
                        if let Stmt::Assign { target: Expr::Field(o, fld), op: "=", value: Expr::Ident(v), .. } = s {
                            if matches!(o.as_ref(), Expr::Ident(x) if x == "self_") && *v == p {
                                field = Some(fld.clone());
                            }
                        }
                    }
                }
            });
        }
        let prefix = class.as_ref().map(|c| format!("{c}_"));
        if let (Some(fld), Some(pre)) = (&field, &prefix) {
            // Read only as a call in the class's methods.
            let other = items.iter().any(|it| match it {
                Item::Fn { name, body, .. } => {
                    let d = format!("{body:?}");
                    // Any other use of the field: read as a value, or set elsewhere.
                    let reads = d.matches(&format!("Field(Ident(\"self_\"), {fld:?})")).count();
                    let set = if *name == f { 1 } else { 0 };
                    let calls = d.matches(&format!("Method(Ident(\"self_\"), {fld:?},")).count();
                    (!name.starts_with(pre.as_str()) && calls > 0) || reads > set
                }
                _ => false,
            });
            if other {
                continue;
            }
        } else {
            // A plain function: p only ever called.
            let Item::Fn { body, .. } = &items[fi] else { continue };
            let d = format!("{body:?}");
            if d.matches(&format!("Ident({p:?})")).count() > 0 || d.matches(&format!("Call({p:?},")).count() == 0 {
                continue;
            }
        }
        // Rewrite.
        for it in items.iter_mut() {
            let Item::Fn { name, params, body, .. } = it else { continue };
            let own = *name == f;
            let in_class = prefix.as_ref().is_some_and(|pre| name.starts_with(pre.as_str()));
            if !own && !in_class {
                continue;
            }
            if own {
                params.retain(|q| q.name != p);
            }
            walk_mut(body, &mut |e| {
                if let Expr::Block(st, _) = e {
                    st.retain(|s| !matches!(s, Stmt::Assign { target: Expr::Field(o, fl), value: Expr::Ident(v), .. } if matches!(o.as_ref(), Expr::Ident(x) if x == "self_") && Some(fl) == field.as_ref() && *v == p));
                }
                let to = match e {
                    Expr::Method(o, m, args, line) if matches!(o.as_ref(), Expr::Ident(x) if x == "self_") && Some(&*m) == field.as_ref() => Some((args.clone(), *line)),
                    Expr::Call(c, args, line) if own && field.is_none() && *c == p => Some((args.iter().map(|(_, a)| a.clone()).collect(), *line)),
                    _ => None,
                };
                if let Some((args, line)) = to {
                    *e = Expr::Call("print".into(), args.into_iter().map(|a| (None, a)).collect(), line);
                }
            });
        }
        if let Some(sig) = sigs.get_mut(&f) {
            sig.0.remove(i);
            sig.1.remove(i);
        }
    }
}


// A parameter only ever called (`got = take(sku, n)`) that every call fills with a bound method
// (`orders.fill(sku, stock.take)`): the call passes the object instead, and `take(a, b)` calls its method.
fn bound_callbacks(items: &mut [Item], script: &mut [Stmt]) {
    let fns: Vec<(String, Vec<String>)> = items.iter().filter_map(|it| match it {
        Item::Fn { name, params, .. } => Some((name.clone(), params.iter().map(|q| q.name.clone()).collect())),
        _ => None,
    }).collect();
    for (f, ps) in &fns {
        let method = ps.first().is_some_and(|p| p == "self_");
        let mname = if method { f.split_once('_').map(|(_, m)| m.to_string()) } else { None };
        // A method name shared by two classes: calls can't be told apart.
        if let Some(m) = &mname {
            if fns.iter().filter(|(g, qs)| g.ends_with(&format!("_{m}")) && qs.first().is_some_and(|q| q == "self_")).count() != 1 {
                continue;
            }
        }
        let Some(fi) = items.iter().position(|it| matches!(it, Item::Fn { name, .. } if name == f)) else { continue };
        for (pi, p) in ps.iter().enumerate() {
            if method && pi == 0 {
                continue;
            }
            let Item::Fn { body, .. } = &items[fi] else { continue };
            let d = format!("{body:?}");
            if d.contains(&format!("Ident({p:?})")) || d.contains(&format!("Move({p:?})")) || !d.contains(&format!("Call({p:?},")) {
                continue;
            }
            // Every call's argument there: `obj.meth`, the same meth each time.
            let ai = if method { pi - 1 } else { pi };
            let (mut calls, mut meth, mut ok) = (0, None::<String>, true);
            let mut check = |e: &Expr| {
                let args: Vec<&Expr> = match e {
                    Expr::Call(c, a, _) if c == f => a.iter().map(|(_, x)| x).collect::<Vec<_>>().into_iter().skip(if method { 1 } else { 0 }).collect(),
                    Expr::Method(_, m, a, _) if Some(m) == mname.as_ref() => a.iter().collect(),
                    _ => return,
                };
                calls += 1;
                match args.get(ai) {
                    Some(Expr::Field(_, m)) if meth.as_ref().map_or(true, |x| x == m) => meth = Some(m.clone()),
                    _ => ok = false,
                }
            };
            for it in items.iter() {
                if let Item::Fn { body, .. } = it {
                    crate::infer::walk(body, &mut check);
                }
            }
            crate::infer::walk(&Expr::Block(script.to_vec(), None), &mut check);
            let (true, Some(meth)) = (ok && calls > 0, meth) else { continue };
            let fix = |e: &mut Expr| {
                let arg = match e {
                    Expr::Call(c, a, _) if c == f => a.get_mut(ai + if method { 1 } else { 0 }).map(|x| &mut x.1),
                    Expr::Method(_, m, a, _) if Some(&*m) == mname.as_ref() => a.get_mut(ai),
                    _ => None,
                };
                if let Some(x) = arg {
                    if let Expr::Field(r, _) = x {
                        *x = (**r).clone();
                    }
                }
            };
            for it in items.iter_mut() {
                if let Item::Fn { body, .. } = it {
                    walk_mut(body, &mut |e| fix(e));
                }
            }
            let mut sc = Expr::Block(script.to_vec(), None);
            walk_mut(&mut sc, &mut |e| fix(e));
            if let Expr::Block(st, _) = sc {
                script.clone_from_slice(&st);
            }
            if let Item::Fn { body, .. } = &mut items[fi] {
                walk_mut(body, &mut |e| {
                    if let Expr::Call(c, a, line) = e {
                        if c == p {
                            *e = Expr::Method(Box::new(Expr::Ident(p.clone())), meth.clone(), a.iter().map(|(_, x)| x.clone()).collect(), *line);
                        }
                    }
                });
            }
        }
    }
}

// A class given `sys.stdout` by every call (`Interp(funcs, sys.stdout)`) that keeps it in a field used only
// as `self.out.write(s)` (and `flush()`): the field goes, and `self.out.write(s)` prints s as it is.
fn stdout_params(items: &mut [Item], script: &mut [Stmt], sigs: &mut HashMap<String, Sig>) {
    let is_out = |v: &Expr| matches!(v, Expr::Field(m, f) if f == "stdout" && matches!(m.as_ref(), Expr::Ident(s) if s == "sys"));
    let names: Vec<(String, Vec<String>)> = items.iter().filter_map(|it| match it {
        Item::Fn { name, params, .. } if name.ends_with("_new") => Some((name.clone(), params.iter().map(|q| q.name.clone()).collect())),
        _ => None,
    }).collect();
    for (f, pnames) in names {
        let Some(class) = f.strip_suffix("_new").map(String::from) else { continue };
        if sigs.get(&f).is_some_and(|s| s.2.is_some() || s.0.len() != pnames.len()) {
            continue;
        }
        for i in 0..pnames.len() {
            let p = pnames[i].clone();
            // Every call passes sys.stdout there, by position.
            let (mut calls, mut all) = (0, true);
            let mut check = |e: &Expr| {
                if let Expr::Call(c, args, _) = e {
                    if *c == f || *c == class {
                        calls += 1;
                        let ok = args.get(i).is_some_and(|(n, v)| n.is_none() && is_out(v)) && args[..i].iter().all(|(n, _)| n.is_none());
                        all &= ok;
                    }
                }
            };
            for it in items.iter() {
                if let Item::Fn { body, .. } = it {
                    crate::infer::walk(body, &mut check);
                }
            }
            crate::infer::walk(&Expr::Block(script.to_vec(), None), &mut check);
            if calls == 0 || !all {
                continue;
            }
            let Some(fi) = items.iter().position(|it| matches!(it, Item::Fn { name, .. } if *name == f)) else { continue };
            let mut field: Option<String> = None;
            if let Item::Fn { body, .. } = &items[fi] {
                crate::infer::walk(body, &mut |e| {
                    if let Expr::Block(st, _) = e {
                        for s in st {
                            if let Stmt::Assign { target: Expr::Field(o, fld), op: "=", value: Expr::Ident(v), .. } = s {
                                if matches!(o.as_ref(), Expr::Ident(x) if x == "self_") && *v == p {
                                    field = Some(fld.clone());
                                }
                            }
                        }
                    }
                });
            }
            let Some(fld) = field else { continue };
            let pre = format!("{class}_");
            // The field read only as `self.F.write(..)` / `self.F.flush()` in the class, nowhere else; p used once.
            let recv = format!("Method(Field(Ident(\"self_\"), {fld:?}), ");
            let read = format!("Field(Ident(\"self_\"), {fld:?})");
            let bad = items.iter().any(|it| match it {
                Item::Fn { name, body, .. } => {
                    let d = format!("{body:?}");
                    let uses = d.matches(&read).count();
                    let ok = d.matches(&format!("{recv}\"write\"")).count() + d.matches(&format!("{recv}\"flush\"")).count() + if *name == f { 1 } else { 0 };
                    (!name.starts_with(pre.as_str()) && uses > 0) || uses > ok || (*name == f && d.matches(&format!("Ident({p:?})")).count() > 1)
                }
                _ => false,
            });
            if bad {
                continue;
            }
            for it in items.iter_mut() {
                let Item::Fn { name, params, body, .. } = it else { continue };
                if *name == f {
                    params.retain(|q| q.name != p);
                }
                let mine = name.starts_with(pre.as_str());
                walk_mut(body, &mut |e| {
                    let flush = |x: &Expr| matches!(x, Expr::Method(o, m, a, _) if m == "flush" && a.is_empty() && matches!(o.as_ref(), Expr::Field(s, fl) if *fl == fld && matches!(s.as_ref(), Expr::Ident(x) if x == "self_")));
                    if let Expr::Block(st, tail) = e {
                        st.retain(|s| !matches!(s, Stmt::Assign { target: Expr::Field(o, fl), value: Expr::Ident(v), .. } if matches!(o.as_ref(), Expr::Ident(x) if x == "self_") && *fl == fld && *v == p));
                        st.retain(|s| !matches!(s, Stmt::Expr { expr, .. } if flush(expr)));
                        if tail.as_deref().is_some_and(flush) {
                            *tail = None;
                        }
                    }
                    if mine {
                        if let Expr::Method(o, m, args, line) = e {
                            if matches!(o.as_ref(), Expr::Field(s, fl) if *fl == fld && matches!(s.as_ref(), Expr::Ident(x) if x == "self_")) {
                                if m == "write" && args.len() == 1 {
                                    *e = Expr::Call("print".into(), vec![(None, args[0].clone()), (Some("end".into()), Expr::Str(vec![]))], *line);
                                }
                            }
                        }
                    }
                    if let Expr::Call(c, args, _) = e {
                        if (*c == f || *c == class) && args.len() > i {
                            args.remove(i);
                        }
                    }
                });
            }
            let mut sc = Expr::Block(script.to_vec(), None);
            walk_mut(&mut sc, &mut |e| {
                if let Expr::Call(c, args, _) = e {
                    if (*c == f || *c == class) && args.len() > i {
                        args.remove(i);
                    }
                }
            });
            if let Expr::Block(st, _) = sc {
                script.clone_from_slice(&st);
            }
            if let Some(sig) = sigs.get_mut(&f) {
                sig.0.remove(i);
                sig.1.remove(i);
            }
            break;
        }
    }
}

// Python's `_old_end = end_tx` at the top level, set once, with end_tx a function of the program: the name
// is another name for the function, so `_old_end()` calls end_tx (also inside functions defined above it).
fn fn_aliases(items: &mut [Item], script: &mut Vec<Stmt>) {
    let fns: Vec<String> = items.iter().filter_map(|it| if let Item::Fn { name, .. } = it { Some(name.clone()) } else { None }).collect();
    let mut i = 0;
    while i < script.len() {
        let (x, f) = match &script[i] {
            Stmt::Assign { target: Expr::Ident(x), op: "=", value: Expr::Ident(f), .. } | Stmt::Let { name: Pat::Bind(x), value: Expr::Ident(f), .. } if fns.contains(f) && !fns.contains(x) && x != f => (x.clone(), f.clone()),
            _ => {
                i += 1;
                continue;
            }
        };
        // Set nowhere else, and no parameter or local of that name.
        let bad = [format!("target: Ident({x:?})"), format!("Bind({x:?})"), format!("name: {x:?}"), format!("{x:?}.into()")];
        let others = script.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, st)| format!("{st:?}")).chain(items.iter().map(|it| format!("{it:?}")));
        let mut clash = false;
        for t in others {
            if bad.iter().any(|b| t.contains(b.as_str())) {
                clash = true;
                break;
            }
        }
        if clash {
            i += 1;
            continue;
        }
        let mut fix = |e: &mut Expr| match e {
            Expr::Call(c, _, _) if *c == x => *c = f.clone(),
            Expr::Ident(c) | Expr::Move(c) if *c == x => *c = f.clone(),
            _ => {}
        };
        for it in items.iter_mut() {
            if let Item::Fn { body, .. } = it {
                walk_mut(body, &mut fix);
            }
        }
        script.remove(i);
        for st in script.iter_mut() {
            match st {
                Stmt::Let { value: e, .. } | Stmt::Assign { value: e, .. } | Stmt::Expr { expr: e, .. } | Stmt::Return { value: Some(e), .. } => walk_mut(e, &mut fix),
                _ => {}
            }
        }
    }
}
