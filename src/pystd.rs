// Standard-library functions for the typed build of a Python file, written in Python over what the typed
// build already has (so they take its types: ints, floats, big ints past 64 bits):
//
//     from math import gcd, pi             import math
//     print(gcd(a, b) * pi)        ->      print(__bv_math_gcd(a, b) * math.pi)
//                                          ...
//                                          def __bv_math_gcd(a, b): ...    (appended, after the last line)
//
// A name taken by `from M import x` becomes `M.x` (unless the program sets that name itself), and `M.f(`
// of a function written here calls the copy appended to the file. Lines keep their numbers. The other
// functions of these modules (math.sqrt, math.log, ...) the parser maps to the typed build's own methods.
use crate::dynpy::parse::{self, Ex, St, Stmt};
use crate::typed_guard::{each_block, walk_st};
use std::collections::{HashMap, HashSet};

// (module, name, Python source of `def __bv_<module>_<name>`).
const SHIMS: &[(&str, &str, &str)] = &[
    ("math", "gcd", "def __bv_math_gcd(a, b):\n    a = abs(a)\n    b = abs(b)\n    while b:\n        a, b = b, a % b\n    return a\n"),
    ("math", "lcm", "def __bv_math_lcm(a, b):\n    if a == 0 or b == 0:\n        return 0\n    x = abs(a)\n    y = abs(b)\n    g = x\n    h = y\n    while h:\n        g, h = h, g % h\n    return x // g * y\n"),
    (
        "math",
        "comb",
        "def __bv_math_comb(n, k):\n    if n < 0 or k < 0:\n        raise ValueError(\"n must be a non-negative integer\")\n    if k > n:\n        return 0\n    k = min(k, n - k)\n    r = 1\n    for i in range(k):\n        r = r * (n - i) // (i + 1)\n    return r\n",
    ),
    (
        "math",
        "perm",
        "def __bv_math_perm(n, k):\n    if n < 0 or k < 0:\n        raise ValueError(\"n must be a non-negative integer\")\n    if k > n:\n        return 0\n    r = 1\n    for i in range(k):\n        r = r * (n - i)\n    return r\n",
    ),
    (
        "math",
        "factorial",
        "def __bv_math_factorial(n):\n    if n < 0:\n        raise ValueError(\"factorial() not defined for negative values\")\n    r = 1\n    for i in range(2, n + 1):\n        r = r * i\n    return r\n",
    ),
    (
        "math",
        "isclose",
        "def __bv_math_isclose(a, b, rel_tol=1e-09, abs_tol=0.0):\n    if a == b:\n        return True\n    d = abs(a - b)\n    return d <= abs(rel_tol * b) or d <= abs(rel_tol * a) or d <= abs_tol\n",
    ),
    // Functions with a domain: ValueError as math raises it, which an except can catch (the typed build's own
    // methods behind them could only stop the program).
    ("math", "sqrt", "def __bv_math_sqrt(x):\n    if x < 0.0:\n        raise ValueError(\"math domain error\")\n    return math.sqrt(x)\n"),
    ("math", "log", "def __bv_math_log(x):\n    if x <= 0.0:\n        raise ValueError(\"math domain error\")\n    return math.log(x)\n"),
    (
        "math",
        "logb",
        "def __bv_math_logb(x, b):\n    if x <= 0.0 or b <= 0.0:\n        raise ValueError(\"math domain error\")\n    if b == 1:\n        raise ZeroDivisionError(\"float division by zero\")\n    return math.log(x, b)\n",
    ),
    ("math", "log2", "def __bv_math_log2(x):\n    if x <= 0.0:\n        raise ValueError(\"math domain error\")\n    return math.log2(x)\n"),
    ("math", "log10", "def __bv_math_log10(x):\n    if x <= 0.0:\n        raise ValueError(\"math domain error\")\n    return math.log10(x)\n"),
    ("math", "log1p", "def __bv_math_log1p(x):\n    if x <= -1.0:\n        raise ValueError(\"math domain error\")\n    return math.log1p(x)\n"),
    ("math", "asin", "def __bv_math_asin(x):\n    if x < -1.0 or x > 1.0:\n        raise ValueError(\"math domain error\")\n    return math.asin(x)\n"),
    ("math", "acos", "def __bv_math_acos(x):\n    if x < -1.0 or x > 1.0:\n        raise ValueError(\"math domain error\")\n    return math.acos(x)\n"),
    ("math", "fmod", "def __bv_math_fmod(x, y):\n    if y == 0.0:\n        raise ValueError(\"math domain error\")\n    return math.fmod(x, y)\n"),
];

// Names of these modules the parser maps itself (math.sqrt, math.pi): a from-import of one is rewritten too.
const MAPPED: &[(&str, &str)] = &[
    ("math", "sqrt"),
    ("math", "floor"),
    ("math", "ceil"),
    ("math", "isqrt"),
    ("math", "pi"),
    ("math", "e"),
    ("math", "tau"),
    ("math", "inf"),
    ("math", "nan"),
    ("math", "log"),
    ("math", "log2"),
    ("math", "log10"),
    ("math", "log1p"),
    ("math", "exp"),
    ("math", "expm1"),
    ("math", "sin"),
    ("math", "cos"),
    ("math", "tan"),
    ("math", "asin"),
    ("math", "acos"),
    ("math", "atan"),
    ("math", "atan2"),
    ("math", "sinh"),
    ("math", "cosh"),
    ("math", "tanh"),
    ("math", "hypot"),
    ("math", "fabs"),
    ("math", "trunc"),
    ("math", "degrees"),
    ("math", "radians"),
    ("math", "copysign"),
    ("math", "fmod"),
    ("math", "isnan"),
    ("math", "isinf"),
    ("math", "isfinite"),
    ("time", "time"),
    ("time", "perf_counter"),
    ("time", "monotonic"),
];

fn shim(m: &str, n: &str) -> Option<&'static str> {
    SHIMS.iter().find(|s| s.0 == m && s.1 == n).map(|s| s.2)
}

fn known(m: &str, n: &str) -> bool {
    shim(m, n).is_some() || MAPPED.contains(&(m, n))
}

// `out = sys.stdout` whose only use is `csv.writer(out, ...)`: the writer takes sys.stdout itself, which the
// typed build has, and the assignment line becomes `pass` (lines keep their numbers).
fn stdout_alias(src: &str) -> Option<String> {
    if !src.contains("sys.stdout") || !src.contains("csv.writer(") {
        return None;
    }
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    let words = |n: &str| {
        let mut k = 0;
        for (i, _) in src.match_indices(n) {
            let before = src[..i].chars().next_back().map_or(false, ident);
            let after = src[i + n.len()..].chars().next().map_or(false, ident);
            if !before && !after {
                k += 1;
            }
        }
        k
    };
    let mut lines: Vec<String> = src.lines().map(String::from).collect();
    let mut changed = false;
    for i in 0..lines.len() {
        let t = lines[i].trim().to_string();
        let Some((n, rhs)) = t.split_once('=') else { continue };
        let n = n.trim().to_string();
        let n = n.as_str();
        if rhs.trim() != "sys.stdout" || n.is_empty() || !n.chars().all(ident) || n.chars().next().unwrap().is_ascii_digit() {
            continue;
        }
        let calls = src.matches(&format!("csv.writer({n},")).count() + src.matches(&format!("csv.writer({n})")).count();
        if calls == 0 || words(n) != 1 + calls {
            continue;
        }
        let indent: String = lines[i].chars().take_while(|c| c.is_whitespace()).collect();
        lines[i] = format!("{indent}pass");
        for l in lines.iter_mut() {
            *l = l.replace(&format!("csv.writer({n},"), "csv.writer(sys.stdout,").replace(&format!("csv.writer({n})"), "csv.writer(sys.stdout)");
        }
        changed = true;
    }
    if !changed {
        return None;
    }
    let mut out = lines.join("\n");
    if src.ends_with('\n') {
        out.push('\n');
    }
    Some(out)
}

pub fn rewrite(src: &str) -> String {
    let aliased = stdout_alias(src);
    let src = aliased.as_deref().unwrap_or(src);
    if !SHIMS.iter().any(|s| src.contains(s.0)) && !MAPPED.iter().any(|s| src.contains(s.0)) {
        return src.to_string();
    }
    let Ok(body) = parse::parse(src) else { return src.to_string() };
    // Modules imported plainly (`import math`, not `import math as m`).
    let mut plain: HashSet<String> = HashSet::new();
    // alias -> (module, name) of `from M import name as alias`, at the top level.
    let mut froms: HashMap<String, (String, String)> = HashMap::new();
    let mut from_lines: Vec<(usize, String, Vec<(String, String)>)> = vec![];
    for s in &body {
        match &s.st {
            St::Import(v) => {
                for (m, a) in v {
                    if a.as_deref().map_or(true, |a| a == m) {
                        plain.insert(m.clone());
                    }
                }
            }
            St::ImportFrom(m, names) => {
                let v: Vec<(String, String)> = names.iter().map(|(n, a)| (n.clone(), a.clone().unwrap_or_else(|| n.clone()))).collect();
                from_lines.push((s.line, m.clone(), v));
            }
            _ => {}
        }
    }
    // A name the program binds itself (a def, an assignment, a parameter...) is left alone.
    let mut bound: HashSet<String> = HashSet::new();
    each_stmt(&body, &mut |s| bound_names(&s.st, &mut bound));
    let mut lines: Vec<String> = src.lines().map(String::from).collect();
    for (line, m, names) in &from_lines {
        // Only a one-line `from M import a, b` all of whose names are written here is rewritten.
        let Some(l) = lines.get(line - 1) else { continue };
        if !l.trim_start().starts_with("from ") || l.contains('(') || l.contains('\\') || l.contains(';') {
            continue;
        }
        if names.iter().any(|(n, a)| !known(m, n) || bound.contains(a) || n == "*") {
            continue;
        }
        let indent: String = l.chars().take_while(|c| c.is_whitespace()).collect();
        if !indent.is_empty() {
            continue;
        }
        lines[line - 1] = format!("import {m}");
        plain.insert(m.clone());
        for (n, a) in names {
            froms.insert(a.clone(), (m.clone(), n.clone()));
        }
    }
    let mut used: Vec<(&str, &str)> = vec![];
    let mut changed = !froms.is_empty();
    for l in lines.iter_mut() {
        let new = rename_line(l, &froms, &plain, &mut used);
        if new != *l {
            *l = new;
            changed = true;
        }
    }
    if !changed {
        return src.to_string();
    }
    let mut out = lines.join("\n");
    out.push('\n');
    used.sort();
    used.dedup();
    let first = lines.len() + 1;
    for (m, n) in used {
        out.push('\n');
        out.push_str(shim(m, n).unwrap());
    }
    SHIM_LINES.with(|x| x.set(Some((first, out.lines().count()))));
    out
}

thread_local! {
    // The lines of the main file the appended copies take (first, last).
    static SHIM_LINES: std::cell::Cell<Option<(usize, usize)>> = const { std::cell::Cell::new(None) };
}

// The generated Rust with the appended copies' line marks dropped: an error raised in one (math domain
// error) names the line of the program's call, not a line past the end of the file.
pub fn drop_shim_lines(rust: String) -> String {
    let Some((a, b)) = SHIM_LINES.with(|x| x.get()) else { return rust };
    // Line 0 marks (a conversion the parser made, `x < 0.0` on an int) would hide the caller's line too.
    let mut out = rust.replace("ax_l(0);", "");
    for l in a..=b {
        out = out.replace(&format!("/*@L{l}*/ax_l({l});"), "").replace(&format!("ax_l({l});"), "");
    }
    out
}

fn each_stmt(body: &[Stmt], f: &mut dyn FnMut(&Stmt)) {
    for s in body {
        f(s);
        match &s.st {
            St::Def(d) => each_stmt(&d.body, f),
            St::Class(c) => each_stmt(&c.body, f),
            st => each_block(st, &mut |b| each_stmt(b, f)),
        }
    }
}

fn bound_names(st: &St, out: &mut HashSet<String>) {
    fn targets(e: &Ex, out: &mut HashSet<String>) {
        match e {
            Ex::Name(n) => {
                out.insert(n.clone());
            }
            Ex::Tuple(v) | Ex::List(v) => v.iter().for_each(|x| targets(x, out)),
            Ex::Star(x) => targets(x, out),
            _ => {}
        }
    }
    match st {
        St::Def(d) => {
            out.insert(d.name.clone());
            for (p, _) in d.params.pos.iter().chain(d.params.kwonly.iter()) {
                out.insert(p.clone());
            }
            out.extend(d.params.star.iter().cloned());
            out.extend(d.params.kwstar.iter().cloned());
        }
        St::Class(c) => {
            out.insert(c.name.clone());
        }
        St::Assign(ts, _) => ts.iter().for_each(|t| targets(t, out)),
        St::AugAssign(t, _, _) | St::AnnAssign(t, _, _) => targets(t, out),
        St::For(t, _, _, _) => targets(t, out),
        _ => {}
    }
    // Comprehension variables, lambda parameters and `:=` names.
    walk_st(st, &mut |e| match e {
        Ex::Comp(_, _, _, fors) => fors.iter().for_each(|c| targets(&c.target, out)),
        Ex::Lambda(p, _) => {
            for (n, _) in p.pos.iter().chain(p.kwonly.iter()) {
                out.insert(n.clone());
            }
        }
        Ex::Walrus(n, _) => {
            out.insert(n.clone());
        }
        _ => {}
    });
}

// One line with names renamed outside strings and comments: a from-imported `gcd` becomes `math.gcd`, and
// `math.gcd(` of a function written here `__bv_math_gcd(`.
fn rename_line<'a>(l: &str, froms: &HashMap<String, (String, String)>, plain: &HashSet<String>, used: &mut Vec<(&'a str, &'a str)>) -> String {
    let cs: Vec<char> = l.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '#' {
            out.extend(&cs[i..]);
            break;
        }
        if c == '"' || c == '\'' {
            // A string to its end on this line (a triple-quoted one spanning lines is left as is after it).
            let start = i;
            let triple = cs.get(i + 1) == Some(&c) && cs.get(i + 2) == Some(&c);
            i += if triple { 3 } else { 1 };
            while i < cs.len() {
                if cs[i] == '\\' {
                    i += 2;
                    continue;
                }
                if cs[i] == c && (!triple || (cs.get(i + 1) == Some(&c) && cs.get(i + 2) == Some(&c))) {
                    i += if triple { 3 } else { 1 };
                    break;
                }
                i += 1;
            }
            if triple && i >= cs.len() {
                // An unclosed triple quote: the rest of the file may be inside it; leave this line alone.
                return l.to_string();
            }
            out.extend(&cs[start..i.min(cs.len())]);
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_') {
                i += 1;
            }
            let w: String = cs[start..i].iter().collect();
            // String prefixes (f"...", r'...'): the word is part of the literal that follows.
            if i < cs.len() && (cs[i] == '"' || cs[i] == '\'') && w.len() <= 2 && w.chars().all(|c| "rRbBfFuU".contains(c)) {
                out.push_str(&w);
                continue;
            }
            let after_dot = out.trim_end().ends_with('.') && !out.trim_end().ends_with("..");
            let next = cs[i..].iter().find(|c| !c.is_whitespace()).copied();
            let mut word = w.clone();
            let mut name: Option<(String, String)> = None;
            if !after_dot {
                if let Some((m, n)) = froms.get(&w) {
                    // `gcd=3` as a keyword argument keeps its name.
                    if !(next == Some('=') && cs[i..].iter().filter(|c| !c.is_whitespace()).nth(1) != Some(&'=')) {
                        word = format!("{m}.{n}");
                        name = Some((m.clone(), n.clone()));
                    }
                } else if plain.contains(&w) && next == Some('.') {
                    // `math.gcd`: read the attribute after the dot.
                    let mut j = i;
                    while j < cs.len() && cs[j].is_whitespace() {
                        j += 1;
                    }
                    j += 1;
                    while j < cs.len() && cs[j].is_whitespace() {
                        j += 1;
                    }
                    let a0 = j;
                    while j < cs.len() && (cs[j].is_alphanumeric() || cs[j] == '_') {
                        j += 1;
                    }
                    let attr: String = cs[a0..j].iter().collect();
                    if !attr.is_empty() {
                        i = j;
                        word = format!("{w}.{attr}");
                        name = Some((w.clone(), attr));
                    }
                }
            }
            if let Some((m, mut n)) = name {
                let called = cs[i..].iter().find(|c| !c.is_whitespace()) == Some(&'(');
                // math.log(x, b) is math.logb's copy.
                if m == "math" && n == "log" && called && args_on_line(&cs[i..]) == Some(2) {
                    n = "logb".into();
                }
                if let Some(s) = SHIMS.iter().find(|s| s.0 == m && s.1 == n) {
                    if called {
                        used.push((s.0, s.1));
                        word = format!("__bv_{m}_{n}");
                    }
                }
            }
            out.push_str(&word);
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

// The arguments of the call starting at the `(` ahead (spaces first), when it closes on this line.
fn args_on_line(cs: &[char]) -> Option<usize> {
    let start = cs.iter().position(|c| *c == '(')?;
    let (mut depth, mut n, mut any) = (0, 1, false);
    let mut q: Option<char> = None;
    for &c in &cs[start..] {
        if let Some(x) = q {
            if c == x {
                q = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                q = Some(c);
                any = true;
            }
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(if any { n } else { 0 });
                }
            }
            ',' if depth == 1 => n += 1,
            c if !c.is_whitespace() => any = true,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::rewrite;

    #[test]
    fn from_import_and_shims() {
        let src = "from math import gcd, pi\nx = gcd(4, 6) * pi\nprint(\"gcd(\", x)  # gcd(\n";
        let out = rewrite(src);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "import math");
        assert_eq!(lines[1], "x = __bv_math_gcd(4, 6) * math.pi");
        assert_eq!(lines[2], "print(\"gcd(\", x)  # gcd(");
        assert!(out.contains("def __bv_math_gcd(a, b):"));
    }

    #[test]
    fn stdout_alias_for_csv_writer() {
        let src = "import csv, sys\ndef f():\n    out = sys.stdout\n    w = csv.writer(out, lineterminator='\\n')\n";
        assert_eq!(super::stdout_alias(src).unwrap(), "import csv, sys\ndef f():\n    pass\n    w = csv.writer(sys.stdout, lineterminator='\\n')\n");
        // Used for something else too: left alone.
        assert!(super::stdout_alias("import csv, sys\nout = sys.stdout\nw = csv.writer(out)\nprint(1, file=out)\n").is_none());
    }

    #[test]
    fn own_names_stay() {
        // The program defines its own gcd: the import line and its uses are left alone.
        let src = "from math import gcd\ndef gcd(a, b):\n    return 1\nprint(gcd(2, 4))\n";
        assert_eq!(rewrite(src), src);
        let src = "import math\nclass P:\n    def f(self):\n        return self.gcd(1)\nprint(math.floor(4.5))\n";
        assert_eq!(rewrite(src), src);
    }

    #[test]
    fn plain_module() {
        let out = rewrite("import math\nprint(math.comb(5, 2), math.floor(2.5))\n");
        assert!(out.starts_with("import math\nprint(__bv_math_comb(5, 2), math.floor(2.5))\n"));
        assert!(out.contains("def __bv_math_comb(n, k):"));
    }
}
