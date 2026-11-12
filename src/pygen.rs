// Endless generators read only with next(), such as a seeded random number source:
//
//     def rng(seed):              class rng:
//         x = seed                    def __init__(self, seed): x = seed; self.seed = seed; self.x = x
//         while True:      ->         def gen_next_(self):
//             x = (x * a + c) % m         seed = self.seed; x = self.x; x = (x * a + c) % m
//             yield x                     self.x = x; return x
//
// and every `r = rng(s)` ... `next(r)` becomes `r.gen_next_()`. A generator sends the whole program to the
// dynamic build, which runs plain loops about as slowly as CPython; the class builds typed. The rewrite keeps
// every line where it was, so errors name the same lines, and it only happens when the generator's state is
// its parameters and the names set before the loop, and its objects are used with next() and nothing else.
use crate::dynpy::parse::{self, Ex, St, Stmt};
use std::collections::BTreeSet;

pub const NEXT: &str = "gen_next_";

pub fn rewrite(src: &str) -> String {
    let Ok(body) = parse::parse(src) else { return src.to_string() };
    let mut lines: Vec<String> = src.lines().map(String::from).collect();
    let mut changed = false;
    for s in &body {
        let St::Def(d) = &s.st else { continue };
        if let Some(new) = one(d, &lines) {
            lines = new;
            changed = true;
        }
    }
    if !changed {
        return src.to_string();
    }
    let mut out = lines.join("\n");
    if src.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn indent(l: &str) -> usize {
    l.len() - l.trim_start().len()
}

fn code_line(l: &str) -> bool {
    let t = l.trim();
    !t.is_empty() && !t.starts_with('#')
}

// The rewritten file, or None when this function is not such a generator.
fn one(d: &parse::FuncDef, lines: &[String]) -> Option<Vec<String>> {
    let p = &d.params;
    if d.is_async || !d.decorators.is_empty() || p.star.is_some() || p.kwstar.is_some() || !p.kwonly.is_empty() || p.pos.iter().any(|(_, def)| def.is_some()) {
        return None;
    }
    let (setup, last) = d.body.split_at(d.body.len().checked_sub(1)?);
    let St::While(cond, wbody, None) = &last[0].st else { return None };
    if !matches!(cond, Ex::True) || setup.is_empty() || wbody.len() < 2 {
        return None;
    }
    let (steps, y) = wbody.split_at(wbody.len() - 1);
    let St::Expr(Ex::Yield(Some(_))) = &y[0].st else { return None };
    // Setup: one simple statement per line, without yields.
    if !setup.iter().all(|s| simple(&s.st) && !has_yield_st(s)) {
        return None;
    }
    if !steps.iter().all(|s| body_ok(s)) || !simple(&steps[0].st) {
        return None;
    }
    let def_l = d.line - 1;
    let while_l = last[0].line - 1;
    let yield_l = y[0].line - 1;
    let first_l = steps[0].line - 1;
    if indent(lines.get(def_l)?) != 0 || !lines[def_l].trim_start().starts_with("def ") {
        return None;
    }
    let setup_lines: Vec<usize> = (def_l + 1..while_l).filter(|&i| code_line(&lines[i])).collect();
    if setup_lines.len() != setup.len() || setup.iter().zip(&setup_lines).any(|(s, &i)| s.line - 1 != i) {
        return None;
    }
    if !lines[while_l].trim().starts_with("while") || first_l <= while_l {
        return None;
    }
    let yl = lines[yield_l].trim();
    let expr = yl.strip_prefix("yield ")?.trim();
    if expr.contains('#') || !balanced(expr) {
        return None;
    }
    // The function ends with the yield line.
    if lines.iter().skip(yield_l + 1).find(|l| code_line(l)).is_some_and(|l| indent(l) > 0) {
        return None;
    }
    let params: Vec<String> = p.pos.iter().map(|(n, _)| n.clone()).collect();
    let mut fields: BTreeSet<String> = params.iter().cloned().collect();
    setup.iter().for_each(|s| assigned(s, &mut fields));
    let mut set_in_loop = BTreeSet::new();
    steps.iter().for_each(|s| assigned(s, &mut set_in_loop));
    if fields.contains("self") || set_in_loop.contains("self") {
        return None;
    }
    let name = &d.name;
    // Every use of the generator is `r = name(...)`, and every use of r is next(r).
    let mut objs = BTreeSet::new();
    for (i, l) in lines.iter().enumerate() {
        if (def_l..=yield_l).contains(&i) {
            continue;
        }
        for _ in words(l, name) {
            let t = l.trim();
            let (lhs, rhs) = t.split_once('=')?;
            let lhs = lhs.trim();
            if !is_ident(lhs) || !rhs.trim_start().starts_with(&format!("{name}(")) || !rhs.trim_end().ends_with(')') || words(rhs, name).len() != 1 {
                return None;
            }
            objs.insert(lhs.to_string());
        }
    }
    if objs.is_empty() || objs.contains(name) {
        return None;
    }
    let mut out = lines.to_vec();
    for (i, l) in lines.iter().enumerate() {
        if (def_l..=yield_l).contains(&i) {
            continue;
        }
        let mut s = l.clone();
        for o in &objs {
            let at = words(&s, o);
            let binding = s.trim().split_once('=').is_some_and(|(a, b)| a.trim() == o && b.trim_start().starts_with(&format!("{name}(")));
            let call = format!("next({o})");
            let calls = s.matches(call.as_str()).count();
            if at.len() != calls + binding as usize {
                return None;
            }
            s = s.replace(&call, &format!("{o}.{NEXT}()"));
        }
        out[i] = s;
    }
    let store = |names: &mut dyn Iterator<Item = &String>| names.map(|n| format!("self.{n} = {n}; ")).collect::<String>();
    out[def_l] = format!("class {name}:");
    let ind1 = " ".repeat(indent(&lines[setup_lines[0]]));
    let joined: Vec<&str> = setup_lines.iter().map(|&i| lines[i].trim()).collect();
    out[setup_lines[0]] = format!("{ind1}def __init__(self, {}): {}; {}", params.join(", "), joined.join("; "), store(&mut fields.iter()).trim_end_matches("; "));
    if params.is_empty() {
        out[setup_lines[0]] = out[setup_lines[0]].replace("(self, )", "(self)");
    }
    for &i in &setup_lines[1..] {
        out[i] = String::new();
    }
    out[while_l] = format!("{}def {NEXT}(self):", " ".repeat(indent(&lines[while_l])));
    let ind2 = " ".repeat(indent(&lines[first_l]));
    let loads: String = fields.iter().map(|n| format!("{n} = self.{n}; ")).collect();
    out[first_l] = format!("{ind2}{loads}{}", lines[first_l].trim_start());
    let kept: Vec<&String> = set_in_loop.iter().filter(|n| fields.contains(*n)).collect();
    out[yield_l] = format!("{}{}return {expr}", " ".repeat(indent(&lines[yield_l])), store(&mut kept.into_iter()));
    Some(out)
}

fn simple(s: &St) -> bool {
    matches!(s, St::Assign(..) | St::AugAssign(..) | St::Expr(_) | St::Pass)
}

fn balanced(s: &str) -> bool {
    let mut d = 0i32;
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => d += 1,
            ')' | ']' | '}' => d -= 1,
            _ => {}
        }
        if d < 0 {
            return false;
        }
    }
    d == 0
}

fn is_ident(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_') && !s.starts_with(|c: char| c.is_ascii_digit())
}

// Byte offsets where the word w starts in l.
fn words(l: &str, w: &str) -> Vec<usize> {
    let b = l.as_bytes();
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    l.match_indices(w).map(|(i, _)| i).filter(|&i| (i == 0 || !word(b[i - 1]) && b[i - 1] != b'.') && b.get(i + w.len()).map_or(true, |&c| !word(c))).collect()
}

fn targets(e: &Ex, out: &mut BTreeSet<String>) {
    match e {
        Ex::Name(n) => {
            out.insert(n.clone());
        }
        Ex::Tuple(v) | Ex::List(v) => v.iter().for_each(|x| targets(x, out)),
        Ex::Star(x) => targets(x, out),
        _ => {}
    }
}

fn assigned(s: &Stmt, out: &mut BTreeSet<String>) {
    match &s.st {
        St::Assign(ts, _) => ts.iter().for_each(|t| targets(t, out)),
        St::AugAssign(t, _, _) | St::AnnAssign(t, _, _, _) => targets(t, out),
        St::For(t, _, b, e) => {
            targets(t, out);
            b.iter().chain(e.iter().flatten()).for_each(|x| assigned(x, out));
        }
        St::While(_, b, e) => b.iter().chain(e.iter().flatten()).for_each(|x| assigned(x, out)),
        St::If(arms, e) => arms.iter().flat_map(|(_, b)| b).chain(e.iter().flatten()).for_each(|x| assigned(x, out)),
        _ => {}
    }
}

// A statement the loop body may hold: no way out of the loop, no yield, nothing that changes scope.
fn body_ok(s: &Stmt) -> bool {
    if has_yield_st(s) {
        return false;
    }
    match &s.st {
        St::Assign(..) | St::AugAssign(..) | St::AnnAssign(..) | St::Expr(_) | St::Pass => true,
        St::If(arms, e) => arms.iter().flat_map(|(_, b)| b).chain(e.iter().flatten()).all(body_ok),
        St::For(_, _, b, None) | St::While(_, b, None) => b.iter().all(|x| body_ok(x) || matches!(x.st, St::Break | St::Continue)),
        _ => false,
    }
}

fn has_yield_st(s: &Stmt) -> bool {
    let mut found = false;
    fn ex(e: &Ex, f: &mut bool) {
        match e {
            Ex::Yield(_) | Ex::YieldFrom(_) | Ex::Await(_) | Ex::Lambda(..) | Ex::Walrus(..) => *f = true,
            Ex::List(v) | Ex::Tuple(v) | Ex::Set(v) => v.iter().for_each(|x| ex(x, f)),
            Ex::Dict(v) => v.iter().for_each(|(k, x)| {
                if let Some(k) = k {
                    ex(k, f)
                }
                ex(x, f)
            }),
            Ex::Star(x) | Ex::Unary(_, x) | Ex::Attr(x, _) => ex(x, f),
            Ex::Index(a, b) | Ex::Bin(_, a, b) | Ex::And(a, b) | Ex::Or(a, b) => {
                ex(a, f);
                ex(b, f)
            }
            Ex::Call(c, args) => {
                ex(c, f);
                args.iter().for_each(|a| match a {
                    parse::Arg::Pos(x) | parse::Arg::Star(x) | parse::Arg::Kw(_, x) | parse::Arg::KwStar(x) => ex(x, f),
                })
            }
            Ex::Cmp(a, v) => {
                ex(a, f);
                v.iter().for_each(|(_, x)| ex(x, f))
            }
            Ex::IfExp(a, b, c) => {
                ex(a, f);
                ex(b, f);
                ex(c, f)
            }
            Ex::Comp(..) => {}
            _ => {}
        }
    }
    match &s.st {
        St::Expr(e) | St::Assign(_, e) | St::AugAssign(_, _, e) => ex(e, &mut found),
        St::If(arms, e) => {
            arms.iter().for_each(|(c, _)| ex(c, &mut found));
            found |= arms.iter().flat_map(|(_, b)| b).chain(e.iter().flatten()).any(has_yield_st);
        }
        St::For(_, it, b, e) => {
            ex(it, &mut found);
            found |= b.iter().chain(e.iter().flatten()).any(has_yield_st);
        }
        St::While(c, b, e) => {
            ex(c, &mut found);
            found |= b.iter().chain(e.iter().flatten()).any(has_yield_st);
        }
        _ => {}
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rng() {
        let src = "def rng(seed):\n    x = seed\n    while True:\n        x = (x * 1103515245 + 12345) % 2147483648\n        yield x\n\nr = rng(3)\nprint(next(r), next(r) % 4)\n";
        let out = rewrite(src);
        assert!(out.contains("class rng:"), "{out}");
        assert!(out.contains("r.gen_next_()"), "{out}");
        assert_eq!(out.lines().count(), src.lines().count());
        // Any other use keeps the generator.
        let other = src.replace("print(next(r), next(r) % 4)", "print(next(r), list(zip(r, range(2))))");
        assert_eq!(rewrite(&other), other);
    }
}
