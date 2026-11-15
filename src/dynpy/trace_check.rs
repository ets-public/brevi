// sys.settrace in dynamic builds: what the build checks before it compiles trace hooks (rt_trace.rs).
//
// A trace function sees an event for every line of every frame entered while it is set, so the guarantee holds
// only if every code body that could run then (each def and class body; module code is never traced, its frame
// being entered before any settrace) produces exactly CPython 3.13's events. The hooks follow CPython for a
// small set of statements, each on one line of its own; a program that uses sys.settrace and holds anything
// else is refused when it is built, never traced differently:
// - one module, importing only sys; no lambda, comprehension, generator, await, or exit()/quit() (Python code
//   in CPython, with its own lines);
// - in a def or class body: expression statements, assignments, augmented assignments, pass, return, raise,
//   assert, if/elif/else, match/case, try/finally (no except), and defs (no annotations, decorators bare
//   names on the lines just above) and classes (no decorators or keywords), each one logical line on one
//   physical line;
// - no dunder method but __init__, __repr__ and __str__ (others run where the runtime calls them), no
//   super() or __class__;
// - of frames and code objects only f_code, f_lineno, co_name and co_firstlineno (read, never assigned); no
//   traceback attributes, sys.exc_info()/sys.exception(), getattr()/setattr()/hasattr()/delattr() but with
//   a literal name, vars(), dir(), locals(), globals().

use super::emit::Module;
use super::parse::{lex, Ex, St, Stmt, Tok};
use std::collections::{HashMap, HashSet};

// One logical line of the source: where it starts and ends, its indentation depth, its first token.
struct LL {
    start: usize,
    end: usize,
    depth: usize,
    first: String,
}

#[derive(Default)]
pub struct TLines {
    lls: Vec<LL>,
    at: HashMap<usize, usize>,
}

impl TLines {
    pub fn new(src: &str) -> Option<TLines> {
        let lx = lex(src).ok()?;
        let mut t = TLines::default();
        let mut depth = 0usize;
        let mut cur: Option<LL> = None;
        for (tk, l) in &lx.toks {
            match tk {
                Tok::Indent => depth += 1,
                Tok::Dedent => depth = depth.saturating_sub(1),
                Tok::Eof => {}
                Tok::Newline => {
                    if let Some(mut c) = cur.take() {
                        c.end = *l;
                        t.at.insert(c.start, t.lls.len());
                        t.lls.push(c);
                    }
                }
                _ => {
                    if cur.is_none() {
                        let first = match tk {
                            Tok::Name(n) => n.clone(),
                            Tok::Op(o) => o.to_string(),
                            _ => "?".into(),
                        };
                        cur = Some(LL { start: *l, end: *l, depth, first });
                    }
                }
            }
        }
        Some(t)
    }
    fn ll(&self, line: usize) -> Option<&LL> {
        self.at.get(&line).map(|&k| &self.lls[k])
    }
    // the logical line starting at `line` is on that one physical line
    pub fn one_line(&self, line: usize) -> bool {
        self.ll(line).is_some_and(|l| l.end == l.start)
    }
    pub fn first(&self, line: usize) -> Option<&str> {
        self.ll(line).map(|l| l.first.as_str())
    }
    // an `if` at `line`: the lines of its elif clauses, and of its else when it has one
    pub fn if_arms(&self, line: usize) -> Option<(Vec<usize>, Option<usize>)> {
        let &k = self.at.get(&line)?;
        let d = self.lls[k].depth;
        let (mut elifs, mut els) = (vec![], None);
        for l in &self.lls[k + 1..] {
            if l.depth > d {
                continue;
            }
            if l.depth < d {
                break;
            }
            match l.first.as_str() {
                "elif" => elifs.push(l.start),
                "else" => {
                    els = Some(l.start);
                    break;
                }
                _ => break,
            }
        }
        Some((elifs, els))
    }
    // a `match` at `line`: the lines of its cases
    pub fn cases(&self, line: usize) -> Option<Vec<usize>> {
        let &k = self.at.get(&line)?;
        let d = self.lls[k].depth;
        let mut out = vec![];
        for l in &self.lls[k + 1..] {
            if l.depth <= d {
                break;
            }
            if l.depth == d + 1 {
                if l.first != "case" {
                    return None;
                }
                out.push(l.start);
            }
        }
        Some(out)
    }
    // a def at `line`: its decorators' lines, when they are the logical lines just above it
    pub fn decorator_lines(&self, line: usize, n: usize) -> Option<Vec<usize>> {
        let &k = self.at.get(&line)?;
        if n > k {
            return None;
        }
        let d = self.lls[k].depth;
        let ds: Vec<usize> = self.lls[k - n..k].iter().filter(|l| l.depth == d && l.first == "@" && l.end == l.start).map(|l| l.start).collect();
        (ds.len() == n).then_some(ds)
    }
}

fn refuse(line: usize, what: &str) -> (usize, String) {
    (line, format!("{what} is not supported in a program that uses sys.settrace in dynamic builds"))
}

const FRAME_ATTRS: [&str; 4] = ["f_code", "f_lineno", "co_name", "co_firstlineno"];
const BANNED_NAMES: [&str; 17] = ["getattr", "setattr", "delattr", "hasattr", "vars", "dir", "locals", "globals", "exec", "eval", "compile", "super", "__class__", "breakpoint", "__import__", "exit", "quit"];
const OK_DUNDERS: [&str; 3] = ["__init__", "__repr__", "__str__"];

fn attr_ok(n: &str) -> bool {
    if FRAME_ATTRS.contains(&n) || n == "__code__" {
        return true;
    }
    !(["f_", "co_", "tb_", "gi_", "cr_", "ag_"].iter().any(|p| n.starts_with(p)) || matches!(n, "__traceback__" | "with_traceback"))
}

fn check_ex(e: &Ex, line: usize) -> Result<(), (usize, String)> {
    let mut bad: Option<String> = None;
    let (mut sys_names, mut sys_attrs) = (0, 0);
    crate::typed_guard::walk_ex(e, &mut |x| {
        if bad.is_some() {
            return;
        }
        match x {
            Ex::Lambda(..) => bad = Some("a lambda".into()),
            Ex::Comp(..) => bad = Some("a comprehension or generator expression".into()),
            Ex::Yield(_) | Ex::YieldFrom(_) => bad = Some("a generator".into()),
            Ex::Await(_) => bad = Some("await".into()),
            Ex::Name(n) if BANNED_NAMES.contains(&n.as_str()) => bad = Some(format!("{n}()")),
            Ex::Name(n) if n == "sys" => sys_names += 1,
            Ex::Attr(o, n) => {
                if !attr_ok(n) {
                    bad = Some(format!("the {n} attribute"));
                } else if matches!(&**o, Ex::Name(m) if m == "sys") {
                    sys_attrs += 1;
                    if matches!(n.as_str(), "exc_info" | "exception") {
                        bad = Some(format!("sys.{n}()"));
                    }
                }
            }
            _ => {}
        }
    });
    if bad.is_none() && sys_names != sys_attrs {
        bad = Some("the sys module as a value".into());
    }
    match bad {
        Some(b) => Err(refuse(line, &b)),
        None => Ok(()),
    }
}

// an assignment or del target: no frame, code or traceback attribute is ever written
fn check_target(t: &Ex, line: usize) -> Result<(), (usize, String)> {
    let mut bad = None;
    crate::typed_guard::walk_ex(t, &mut |x| {
        if let Ex::Attr(_, n) = x {
            if n == "__code__" || ["f_", "co_", "tb_"].iter().any(|p| n.starts_with(p)) || n == "__traceback__" {
                bad = Some(n.clone());
            }
        }
    });
    if let Some(n) = bad {
        return Err(refuse(line, &format!("assigning the {n} attribute")));
    }
    check_ex(t, line)
}

struct Ck<'a> {
    tl: &'a TLines,
}

impl Ck<'_> {
    fn one(&self, line: usize, what: &str) -> Result<(), (usize, String)> {
        if self.tl.one_line(line) {
            Ok(())
        } else {
            Err(refuse(line, &format!("{what} over more than one line (or not on a line of its own)")))
        }
    }
    fn stmts(&self, body: &[Stmt], in_body: bool) -> Result<(), (usize, String)> {
        for s in body {
            self.stmt(s, in_body)?;
        }
        Ok(())
    }
    fn stmt(&self, s: &Stmt, in_body: bool) -> Result<(), (usize, String)> {
        let line = s.line;
        if in_body && !matches!(s.st, St::Def(_)) {
            self.one(line, "a statement")?;
        }
        match &s.st {
            St::Import(names) => {
                if names.iter().any(|(m, a)| m != "sys" || a.is_some()) {
                    return Err(refuse(line, "importing a module other than sys (or sys under another name)"));
                }
                if in_body {
                    return Err(refuse(line, "import inside a def or class"));
                }
            }
            St::ImportFrom(..) => return Err(refuse(line, "from ... import")),
            St::Expr(e) => check_ex(e, line)?,
            St::Assign(ts, v) => {
                for t in ts {
                    check_target(t, line)?;
                }
                check_ex(v, line)?;
            }
            St::AugAssign(t, _, v) => {
                check_target(t, line)?;
                check_ex(v, line)?;
            }
            St::Pass => {}
            St::Return(v) => {
                if let Some(v) = v {
                    check_ex(v, line)?;
                }
            }
            St::Raise(a, b) => {
                for x in a.iter().chain(b.iter()) {
                    check_ex(x, line)?;
                }
            }
            St::Assert(a, b) => {
                for x in std::iter::once(a).chain(b.iter()) {
                    check_ex(x, line)?;
                }
            }
            St::If(arms, els) => {
                for (c, b) in arms {
                    check_ex(c, line)?;
                    self.stmts(b, in_body)?;
                }
                if let Some(b) = els {
                    self.stmts(b, in_body)?;
                }
                if in_body {
                    match self.tl.first(line) {
                        Some("if") => {
                            let (elifs, el) = self.tl.if_arms(line).ok_or_else(|| refuse(line, "this if statement"))?;
                            if elifs.len() + 1 != arms.len() || el.is_some() != els.is_some() {
                                return Err(refuse(line, "this if statement"));
                            }
                            for l in elifs.iter().chain(el.iter()) {
                                self.one(*l, "an elif or else line")?;
                            }
                        }
                        Some("match") => {
                            let cases = self.tl.cases(line).ok_or_else(|| refuse(line, "this match statement"))?;
                            if cases.len() != arms.len() + usize::from(els.is_some()) {
                                return Err(refuse(line, "this match statement"));
                            }
                            for l in &cases {
                                self.one(*l, "a case line")?;
                            }
                        }
                        _ => return Err(refuse(line, "this statement")),
                    }
                }
            }
            St::Try(b, hs, el, fin) if hs.is_empty() && el.is_none() && fin.is_some() => {
                self.stmts(b, in_body)?;
                self.stmts(fin.as_deref().unwrap(), in_body)?;
            }
            St::Def(d) => {
                if d.is_async {
                    return Err(refuse(d.line, "async def"));
                }
                if !d.tparams.is_empty() {
                    return Err(refuse(d.line, "a PEP 695 def"));
                }
                if !d.params.ann.is_empty() || d.ret.is_some() {
                    return Err(refuse(d.line, "an annotated def"));
                }
                if d.name.starts_with("__") && d.name.ends_with("__") && !OK_DUNDERS.contains(&d.name.as_str()) {
                    return Err(refuse(d.line, &format!("defining {}", d.name)));
                }
                self.one(d.line, "a def line")?;
                if d.body.len() == 1 && matches!(&d.body[0].st, St::Expr(Ex::Str(_))) {
                    return Err(refuse(d.line, "a def whose body is only its docstring"));
                }
                for x in &d.decorators {
                    if !matches!(x, Ex::Name(_)) {
                        return Err(refuse(d.line, "a decorator other than a bare name"));
                    }
                    check_ex(x, d.line)?;
                }
                if !d.decorators.is_empty() && self.tl.decorator_lines(d.line, d.decorators.len()).is_none() {
                    return Err(refuse(d.line, "decorators not each on its own line just above the def"));
                }
                for (_, dv) in d.params.pos.iter().chain(d.params.kwonly.iter()) {
                    if let Some(x) = dv {
                        check_ex(x, d.line)?;
                    }
                }
                self.stmts(&d.body, true)?;
            }
            St::Class(c) => {
                if !c.decorators.is_empty() || !c.kwargs.is_empty() {
                    return Err(refuse(c.line, "a class with decorators or keywords"));
                }
                self.one(c.line, "a class line")?;
                for b in &c.bases {
                    check_ex(b, c.line)?;
                }
                self.stmts(&c.body, true)?;
            }
            _ if in_body => return Err(refuse(line, "this statement inside a def or class")),
            // module code is never traced: its own statements may be anything the build takes
            St::While(c, b, e) => {
                check_ex(c, line)?;
                self.stmts(b, false)?;
                if let Some(e) = e {
                    self.stmts(e, false)?;
                }
            }
            St::For(t, it, b, e) => {
                check_target(t, line)?;
                check_ex(it, line)?;
                self.stmts(b, false)?;
                if let Some(e) = e {
                    self.stmts(e, false)?;
                }
            }
            St::Try(b, hs, el, fin) => {
                self.stmts(b, false)?;
                for h in hs {
                    if let Some(t) = &h.types {
                        check_ex(t, h.line)?;
                    }
                    self.stmts(&h.body, false)?;
                }
                for x in el.iter().chain(fin.iter()) {
                    self.stmts(x, false)?;
                }
            }
            St::With(items, b) => {
                for (x, t) in items {
                    check_ex(x, line)?;
                    if let Some(t) = t {
                        check_target(t, line)?;
                    }
                }
                self.stmts(b, false)?;
            }
            St::Del(ts) => {
                for t in ts {
                    check_target(t, line)?;
                }
            }
            St::AnnAssign(..) => return Err(refuse(line, "an annotated assignment")),
            St::Global(_) | St::Nonlocal(_) | St::Break | St::Continue => {}
        }
        Ok(())
    }
}

// Whether the program asks for tracing: any mention of settrace (a name, an attribute, a string).
pub fn wants_trace(mods: &[Module], srcs: &HashSet<String>) -> bool {
    mods.iter().any(|m| srcs.contains(&m.name) && format!("{:?}", m.body).contains("settrace"))
}

pub fn check(mods: &[Module], src: Option<&str>) -> Result<TLines, (usize, String)> {
    if mods.len() != 1 {
        return Err(refuse(1, "importing a module of the program or a Brevi library"));
    }
    let tl = src.and_then(TLines::new).ok_or_else(|| refuse(1, "a program whose source is not at hand"))?;
    Ck { tl: &tl }.stmts(&mods[0].body, false)?;
    Ok(tl)
}
