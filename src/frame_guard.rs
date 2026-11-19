// CPython's locals(), globals(), vars(), dir(), eval(), exec() and breakpoint() read the frame of the code that
// calls them. Called by their own names, every build handles them (or refuses them) itself. Reached through
// the builtins module (`builtins.locals()`, `b = builtins; b.vars()`, `getattr(builtins, "locals")`,
// `from builtins import locals as l`), they would run in CPython from a native frame CPython cannot see: a
// SystemError "frame does not exist" or a run-time NotImplementedError where CPython gives an answer (and in an
// Enum body, the EnumDict's TypeError). So a file that could reach one that way is refused before the typed,
// bridged and dynamic builds, and runs in CPython. The builtins module (and `__builtins__`, `__import__("builtins")`,
// ...) may be used only as `M.name` or `getattr(M, "name")`/`hasattr`/`setattr`/`delattr` with a literal name that
// is not one of these nor a dunder that hands out the namespace; any other use of it (passed, stored, subscripted,
// vars(M), a computed name) could reach them, and is refused. A module reached by a computed name
// (`importlib.import_module(s)`) is not seen here: the bridges refuse those builtins when one comes back from CPython.
use crate::dynpy::parse::{Arg, Ex, Params, St, Stmt};
use crate::typed_guard::walk_ex;
use std::collections::HashSet;

const FRAME: &[&str] = &["locals", "globals", "vars", "dir", "eval", "exec", "breakpoint"];

// Every expression a statement list evaluates, with its statement's line, nested bodies included (defaults and
// annotations of defs and lambdas, decorators, class bases and keywords, raise causes, except types).
fn each_ex(body: &[Stmt], f: &mut dyn FnMut(&Ex, usize)) {
    fn params(p: &Params, line: usize, f: &mut dyn FnMut(&Ex, usize)) {
        for (_, d) in p.pos.iter().chain(p.kwonly.iter()) {
            if let Some(d) = d {
                ex(d, line, f);
            }
        }
        for (_, a) in &p.ann {
            ex(a, line, f);
        }
    }
    fn ex(e: &Ex, line: usize, f: &mut dyn FnMut(&Ex, usize)) {
        walk_ex(e, &mut |x| {
            f(x, line);
            if let Ex::Lambda(p, _) = x {
                params(p, line, f);
            }
        });
    }
    for s in body {
        let line = s.line;
        crate::typed_guard::walk_st(&s.st, &mut |x| {
            f(x, line);
            if let Ex::Lambda(p, _) = x {
                params(p, line, f);
            }
        });
        match &s.st {
            St::AnnAssign(_, _, a, _) => ex(a, line, f),
            // walk_st covers `raise e` itself, not its cause
            St::Raise(_, Some(c)) => ex(c, line, f),
            St::Def(d) => {
                d.decorators.iter().for_each(|x| ex(x, line, f));
                params(&d.params, line, f);
                if let Some(r) = &d.ret {
                    ex(r, line, f);
                }
            }
            St::Class(c) => {
                for x in c.decorators.iter().chain(c.bases.iter()).chain(c.kwargs.iter().map(|(_, x)| x)) {
                    ex(x, line, f);
                }
            }
            St::Try(_, hs, _, _) => {
                for h in hs {
                    if let Some(t) = &h.types {
                        ex(t, h.line, f);
                    }
                }
            }
            _ => {}
        }
        crate::dynpy::typing::blocks(&s.st, &mut |b| each_ex(b, f));
    }
}

fn param_names(p: &Params) -> impl Iterator<Item = &str> {
    p.pos.iter().chain(p.kwonly.iter()).map(|(n, _)| n.as_str()).chain(p.star.as_deref()).chain(p.kwstar.as_deref())
}

fn each_st(body: &[Stmt], f: &mut dyn FnMut(&Stmt)) {
    for s in body {
        f(s);
        crate::dynpy::typing::blocks(&s.st, &mut |b| each_st(b, f));
    }
}

// The first line of a file that could reach a frame-reading builtin through the builtins module, with why.
pub fn check(body: &[Stmt]) -> Option<(usize, String)> {
    let mut mods: HashSet<String> = HashSet::from(["__builtins__".to_string()]);
    let mut hit: Option<(usize, String)> = None;
    each_st(body, &mut |s| match &s.st {
        St::Import(v) => {
            for (m, a) in v {
                if m == "builtins" {
                    mods.insert(a.clone().unwrap_or_else(|| m.clone()));
                }
            }
        }
        St::ImportFrom(m, v) if m == "builtins" && hit.is_none() => {
            if let Some((n, _)) = v.iter().find(|(n, _)| n == "*" || FRAME.contains(&n.as_str())) {
                let what = if n == "*" { "`from builtins import *`".to_string() } else { format!("`from builtins import {n}`") };
                hit = Some((s.line, format!("{what} reads the caller's frame, which a native build does not have")));
            }
        }
        _ => {}
    });
    if hit.is_some() {
        return hit;
    }
    // getattr and the rest count as the builtins only when the file never binds or passes those names
    const GET: &[&str] = &["getattr", "hasattr", "setattr", "delattr"];
    let mut rebound = false;
    let mut callee: HashSet<*const Ex> = HashSet::new();
    each_ex(body, &mut |e, _| {
        if let Ex::Call(fx, _) = e {
            callee.insert(fx.as_ref() as *const Ex);
        }
        if let Ex::Walrus(n, _) = e {
            rebound |= GET.contains(&n.as_str());
        }
        if let Ex::Lambda(p, _) = e {
            rebound |= param_names(p).any(|n| GET.contains(&n));
        }
    });
    each_ex(body, &mut |e, _| rebound |= matches!(e, Ex::Name(n) if GET.contains(&n.as_str())) && !callee.contains(&(e as *const Ex)));
    each_st(body, &mut |s| {
        let names: Vec<&str> = match &s.st {
            St::Def(d) => std::iter::once(d.name.as_str()).chain(param_names(&d.params)).collect(),
            St::Class(c) => vec![c.name.as_str()],
            St::Import(v) | St::ImportFrom(_, v) => v.iter().map(|(m, a)| a.as_deref().unwrap_or(m.as_str())).collect(),
            St::Try(_, hs, _, _) => hs.iter().filter_map(|h| h.name.as_deref()).collect(),
            _ => vec![],
        };
        rebound |= names.iter().any(|n| GET.contains(n) || *n == "*");
    });
    // The builtins module: a name bound to it, `__builtins__`, `__import__("builtins")`,
    // `importlib.import_module("builtins")` or `sys.modules["builtins"]`. The uses of it that are allowed:
    // `M.name` and getattr-style calls with a literal name, where the name is not a frame builtin and not a dunder
    // that hands out the module's namespace (only `M.__dict__.keys()`, names alone, is).
    let is_mod = |e: &Ex| match e {
        Ex::Name(n) => mods.contains(n),
        Ex::Call(fx, a) => {
            matches!(a.first(), Some(Arg::Pos(Ex::Str(m))) if m == "builtins")
                && (matches!(fx.as_ref(), Ex::Name(n) if n == "__import__") || matches!(fx.as_ref(), Ex::Attr(_, n) if n == "import_module"))
        }
        Ex::Index(o, k) => matches!(k.as_ref(), Ex::Str(m) if m == "builtins") && matches!(o.as_ref(), Ex::Attr(_, n) if n == "modules"),
        _ => false,
    };
    let bad = |n: &str, keys: bool| {
        if FRAME.contains(&n) {
            Some(format!("builtins.{n} reads the caller's frame, which a native build does not have"))
        } else if n.starts_with("__") && !matches!(n, "__name__" | "__doc__" | "__package__") && !(n == "__dict__" && keys) {
            Some(format!("builtins.{n} could reach locals(), vars() or another builtin that reads the caller's frame, which a native build does not have"))
        } else {
            None
        }
    };
    let mut ok: HashSet<*const Ex> = HashSet::new();
    let mut keys: HashSet<*const Ex> = HashSet::new();
    each_ex(body, &mut |e, line| {
        if hit.is_some() {
            return;
        }
        match e {
            Ex::Attr(o, n) if is_mod(o) => {
                hit = bad(n, keys.contains(&(e as *const Ex))).map(|w| (line, w));
                ok.insert(o.as_ref() as *const Ex);
            }
            // parents are visited before their children: `M.__dict__` is seen here before its own visit
            Ex::Attr(o, n) if n == "keys" => {
                keys.insert(o.as_ref() as *const Ex);
            }
            Ex::Call(fx, args) if !rebound && matches!(fx.as_ref(), Ex::Name(g) if GET.contains(&g.as_str())) => {
                if let (Some(Arg::Pos(o)), Some(Arg::Pos(Ex::Str(n)))) = (args.first(), args.get(1)) {
                    if is_mod(o) {
                        hit = bad(n, false).map(|w| (line, w));
                        ok.insert(o as *const Ex);
                    }
                }
            }
            _ => {}
        }
    });
    if hit.is_some() {
        return hit;
    }
    each_ex(body, &mut |e, line| {
        if hit.is_none() && is_mod(e) && !ok.contains(&(e as *const Ex)) {
            let n = if let Ex::Name(n) = e { n.as_str() } else { "builtins" };
            hit = Some((line, format!("the builtins module used other than as `{n}.name` could reach locals(), vars() or another builtin that reads the caller's frame, which a native build does not have")));
        }
    });
    hit
}
