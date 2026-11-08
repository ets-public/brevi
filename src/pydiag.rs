// Mistakes in a Python file that are errors in CPython too: a name defined nowhere (`print(totl(nums))`), a
// method no Python type has (`x.uppr()`), text plus a number (`"n=" + len(nums)`). The typed build finds
// these; without this check such a file went on to the dynamic build, which accepted it, and the mistake only
// showed when that line ran. Anything less certain (a name Brevi's typed build does not know, a type it cannot
// work out) still goes to the dynamic build: this never refuses a file CPython would run.
use crate::diag::Diag;
use std::collections::HashSet;

const BUILTINS: &str = include_str!("pynames/builtins.txt");
// Every public method or attribute of Python's built-in types and of the common standard-library modules and
// their classes (generated with dir() on CPython 3.13).
const METHODS: &str = include_str!("pynames/methods.txt");
// Methods of str, list, dict, set, int, float, tuple and bytes, for "did you mean" hints.
const CORE: &str = include_str!("pynames/core.txt");
// The modules METHODS covers.
const MODS: &str = include_str!("pynames/mods.txt");
// Brevi's list of built-in methods has these too, and Python has them on values the typed build keeps as a
// list, map or number (`Counter.most_common`, `timedelta.days`): not sure mistakes on such a value.
const ON_LIBRARY_VALUES: [&str; 9] = ["day", "days", "heapify", "month", "most_common", "move_to_end", "quantize", "weekday", "year"];
// What Python calls Brevi's methods and functions, for the fix of an error about one.
const PY_SPELLING: [(&str, &str); 41] = [
    ("abs", "abs(x)"),
    ("all", "all(xs)"),
    ("any", "any(xs)"),
    ("args", "sys.argv"),
    ("chars", "list(s)"),
    ("contains", "`x in xs`"),
    ("ends_with", ".endswith()"),
    ("enumerate", "enumerate(xs)"),
    ("fail", "raise"),
    ("filter", "a list comprehension"),
    ("first", "xs[0]"),
    ("float", "float(x)"),
    ("fmt", "format(x, \".2f\")"),
    ("get_or", ".get(k, default)"),
    ("has", "`k in m`"),
    ("index_of", ".index()"),
    ("int", "int(x)"),
    ("is_alpha", ".isalpha()"),
    ("is_digit", ".isdigit()"),
    ("is_empty", "`not xs`"),
    ("is_space", ".isspace()"),
    ("join", "sep.join(xs)"),
    ("last", "xs[-1]"),
    ("len", "len(x)"),
    ("map", "a list comprehension"),
    ("max", "max(xs)"),
    ("min", "min(xs)"),
    ("pairs", ".items()"),
    ("push", ".append()"),
    ("read", "open(path).read()"),
    ("rev", "reversed(xs)"),
    ("slice", "xs[a:b]"),
    ("sort", "sorted(s)"),
    ("sort_by", ".sort(key=f)"),
    ("starts_with", ".startswith()"),
    ("stdin", "sys.stdin.read()"),
    ("sum", "sum(xs)"),
    ("take", "xs[:n]"),
    ("trim", ".strip()"),
    ("words", ".split()"),
    ("zip", "zip(xs, ys)"),
];
// Method calls as the program wrote them, before the parser turns Python's names into Brevi's: (line, name,
// receiver). `xs.append(x)` also ends up as `push`, so the name alone does not tell what was written.
static WRITTEN: std::sync::Mutex<Option<HashSet<(usize, String, String)>>> = std::sync::Mutex::new(None);

// Whether Brevi has a built-in method `m` that Python's built-in types do not.
fn brevi_only(m: &str) -> bool {
    crate::emit::METHODS.contains(&m) && !CORE.split_whitespace().any(|x| x == m) && !ON_LIBRARY_VALUES.contains(&m) && !FROM_LEXER.contains(&m)
}

// What the lexer turns `v is True`, `v is not False` and the like into: the program wrote Python, not these.
const FROM_LEXER: &[&str] = &["is_true_obj", "is_false_obj", "not_true_obj", "not_false_obj"];

// Brevi's `xs.join(sep)` and `s.sort()`: Python has these methods, on other types.
fn other_type(m: &str) -> bool {
    matches!(m, "join" | "sort")
}

// The parser read `recv.m(...)` in a Python file.
pub fn note_call(line: usize, m: &str, recv: &crate::ast::Expr) {
    if crate::PY_FILE.load(std::sync::atomic::Ordering::Relaxed) && (brevi_only(m) || other_type(m)) {
        WRITTEN.lock().unwrap().get_or_insert_with(HashSet::new).insert((line, m.to_string(), format!("{recv:?}")));
    }
}

// The error for `recv.m(...)` when recv is a list, a dict, text or a number (`wrong_type`: one that lacks Python's
// `m`) and the program wrote that call itself: CPython raises AttributeError there. A class's own `m` never gets
// here, and neither does a value whose type is not known.
pub fn no_such_builtin(line: usize, m: &str, recv: &crate::ast::Expr, wrong_type: bool) -> Option<Diag> {
    if !brevi_only(m) && !wrong_type {
        return None;
    }
    let written = WRITTEN.lock().unwrap();
    written.as_ref().filter(|w| w.contains(&(line, m.to_string(), format!("{recv:?}"))))?;
    let mut e = Diag::new("check", line, 1, &format!("{NO_BUILTIN}{m}() in Python"));
    e.fix = Some(method_fix(m));
    Some(e)
}

const NO_BUILTIN: &str = "this value has no method .";

fn spelling(m: &str) -> Option<&'static str> {
    PY_SPELLING.iter().find(|(b, _)| *b == m).map(|(_, p)| *p)
}

fn method_fix(m: &str) -> String {
    match spelling(m).map(String::from).or_else(|| crate::emit::closest(m, CORE.split_whitespace()).map(|b| format!(".{b}()"))) {
        Some(p) => format!("did you mean {p}?"),
        None => "check the spelling".to_string(),
    }
}

// Brevi's own syntax in a Python file, which CPython stops on with a SyntaxError before running anything. The
// typed parser takes it, so this looks for it itself: `&&`-style operators anywhere, and any other Brevi form on
// a line the strict parser of the dynamic build refuses. That parser also refuses some valid Python it lacks
// (`def f[T](x):`), so a refusal alone proves nothing.
pub fn not_python(src: &str) -> Option<Diag> {
    // A bare `allow read` line stays: it is how a program holds itself to its capabilities.
    let src = &crate::caps::split(src).0;
    let code = blank(src);
    let refused = crate::dynpy::parse::parse(src).err().filter(|e| !e.1.contains("not supported")).map(|e| e.0);
    code.lines().enumerate().find_map(|(i, l)| {
        let (what, fix) = brevi_operator(l).or_else(|| brevi_form(l).filter(|_| refused == Some(i + 1)))?;
        Some(Diag::new("check", i + 1, 1, what).fix_hint(fix))
    })
}

// An operator of Brevi's on a line with strings and comments blanked: never part of valid Python.
fn brevi_operator(l: &str) -> Option<(&'static str, &'static str)> {
    let b = l.as_bytes();
    Some(if l.contains("&&") {
        ("`&&` is Brevi, not Python", "write `and`")
    } else if l.contains("||") {
        ("`||` is Brevi, not Python", "write `or`")
    } else if l.contains("=>") {
        ("`=>` is Brevi, not Python", "write `lambda x: ...`, or `match` with `case` arms")
    } else if l.contains("~=") {
        ("`~=` is Brevi, not Python", "write `abs(a - b) < 1e-6`")
    } else if (0..b.len()).any(|i| b[i] == b'!' && b.get(i + 1) != Some(&b'=')) {
        ("`!` is Brevi, not Python", "write `not`")
    } else {
        return None;
    })
}

// A Brevi form on a line with strings and comments blanked, for a line that is not valid Python.
fn brevi_form(l: &str) -> Option<(&'static str, &'static str)> {
    let t = l.trim();
    let starts = |w: &str| t.strip_prefix(w).is_some_and(|r| r.starts_with(' '));
    let b = t.as_bytes();
    let tuple_field = (1..b.len()).any(|i| b[i] == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit) && (b[i - 1].is_ascii_alphabetic() || matches!(b[i - 1], b'_' | b')' | b']')));
    // `0..n` can be valid: see float_ranges.
    let range = t.match_indices("..").any(|(p, _)| {
        !t[p + 2..].starts_with('.') && !t[..p].ends_with('.') && !(t[..p].ends_with(|c: char| c.is_ascii_digit()) && t[p + 2..].starts_with(|c: char| c.is_alphabetic() || c == '_'))
    });
    let named_arg = !t.contains("def ") && !t.contains("class ") && !t.contains("lambda") && t.match_indices(": ").any(|(p, _)| {
        let word = t[..p].trim_end_matches(is_word);
        word.len() < p && (word.ends_with('(') || (word.ends_with(", ") && word.contains('(')))
    });
    let block = ["if", "elif", "else", "for", "while"].iter().any(|w| t.starts_with(w)) && t.ends_with('}') && t.contains(" {");
    Some(if starts("fn") {
        ("`fn` is Brevi, not Python", "write `def`")
    } else if starts("let") || starts("var") {
        ("`let` and `var` are Brevi, not Python", "write `x = value`")
    } else if (starts("type") || starts("enum")) && t.ends_with(':') {
        ("`type` and `enum` blocks are Brevi, not Python", "write a `class`")
    } else if starts("test") {
        ("`test` blocks are Brevi, not Python", "write a function with `assert` lines")
    } else if starts("use") {
        ("`use` is Brevi, not Python", "write `import`")
    } else if t.starts_with("else if ") {
        ("`else if` is Brevi, not Python", "write `elif`")
    } else if t.contains("= if ") {
        ("`if` as a value is Brevi, not Python", "write `a if c else b`")
    } else if starts("def") && (t.contains("(mut ") || t.contains(", mut ")) {
        ("`mut` is Brevi, not Python", "leave it out")
    } else if block {
        ("a block in braces is Brevi, not Python", "write `:` and the block on the lines below")
    } else if range {
        ("`a..b` is Brevi, not Python", "write `range(a, b)`")
    } else if tuple_field {
        ("`t.0` is Brevi, not Python", "write `t[0]`")
    } else if t.contains('?') {
        ("`?` is Brevi, not Python", "leave it out: errors in Python pass up as exceptions")
    } else if t.starts_with("//") || t.split_once("//").is_some_and(|(_, r)| crate::lexer::looks_like_prose(r)) {
        ("`//` comments are Brevi, not Python", "write `#`")
    } else if named_arg {
        ("`name: value` arguments are Brevi, not Python", "write `name=value`")
    } else {
        return None;
    })
}

// The errors among `diags` (from the typed build of `src`, a Python file in `dir`) that CPython would also
// raise, reworded the way a Python programmer expects. Empty when none is certain.
pub fn definite(diags: &[Diag], src: &str, dir: &std::path::Path) -> Vec<Diag> {
    let dir = if dir.as_os_str().is_empty() { std::path::Path::new(".") } else { dir };
    let code = blank(src);
    // Names made at run time: nothing about a name is certain.
    if ["import *", "globals()", "locals()", "exec(", "eval(", "setattr(", "__builtins__", "builtins.", "__getattr__", "vars("].iter().any(|m| code.contains(m)) {
        return vec![];
    }
    let stmts = statements(&code);
    let builtins: HashSet<&str> = BUILTINS.split_whitespace().collect();
    let mut out: Vec<Diag> = vec![];
    for d in diags {
        let named = |pre: &str, post: &str| d.msg.strip_prefix(pre).and_then(|r| r.strip_suffix(post)).map(String::from);
        let line_code = code.lines().nth(d.line.wrapping_sub(1)).unwrap_or("");
        if maybe_unreached(&code, d.line) {
            continue;
        }
        let found = if let Some(n) = named("unknown function ", "()").or_else(|| named("unknown name '", "'")) {
            (!caught(&code, "NameError") && undefined(&n, &stmts, &builtins, line_code)).then(|| {
                let words = words(&code);
                let best = spelling(&n).map(String::from).or_else(|| crate::emit::closest(&n, words.iter().map(|s| s.as_str()).filter(|w| *w != n && !w.starts_with("ax_")).chain(builtins.iter().copied())).map(String::from));
                let mut e = Diag::new("check", d.line, 1, &format!("name '{n}' is not defined"));
                e.fix = Some(best.map_or("define it before this line, or check the spelling".to_string(), |b| format!("did you mean {b}?")));
                e
            })
        } else if let Some(m) = named("unknown method .", "()") {
            (!caught(&code, "AttributeError") && no_such_method(&m, &code, dir, line_code)).then(|| {
                let mut e = Diag::new("check", d.line, 1, &format!("no Python type has a method .{m}()"));
                e.fix = Some(method_fix(&m));
                e
            })
        } else if d.msg.starts_with(NO_BUILTIN) {
            // Elsewhere the dynamic build runs the line and raises AttributeError as CPython does.
            (!caught(&code, "AttributeError")).then(|| d.clone())
        } else if d.msg.starts_with("cannot add str and ") || (d.msg.starts_with("cannot add ") && d.msg.ends_with(" and str")) {
            // Only with a string literal right beside a `+` on the line: the text side is not a guess.
            (!caught(&code, "TypeError") && text_plus(&src.lines().nth(d.line.wrapping_sub(1)).unwrap_or(""))).then(|| d.clone())
        } else {
            None
        };
        if let Some(e) = found {
            // `s.trim().len()`: the inner call comes second here and is the one CPython stops on.
            if let Some(o) = out.iter_mut().find(|o| o.line == e.line && o.msg.starts_with(NO_BUILTIN) && e.msg.starts_with(NO_BUILTIN)) {
                *o = e;
            } else if !out.iter().any(|o| o.line == e.line && o.msg == e.msg) {
                out.push(e);
            }
        }
    }
    out
}

// The same mistakes found without the typed build's help, for a file it refused for another reason first (or
// never tried): calls of names defined nowhere, and calls of methods no Python type or common module has.
pub fn scan(src: &str, dir: &std::path::Path) -> Vec<Diag> {
    scan_calls(src, dir, false)
}

// The same for a file the typed build took, which only needs its Brevi spellings found: other methods that
// no Python type has can be real there (tkinter's, which METHODS does not cover).
pub fn scan_brevi(src: &str, dir: &std::path::Path) -> Vec<Diag> {
    scan_calls(src, dir, true)
}

fn scan_calls(src: &str, dir: &std::path::Path, brevi: bool) -> Vec<Diag> {
    const KEYWORDS: [&str; 35] = [
        "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield", "match", "case", "None",
    ];
    let code = blank(src);
    let mut diags = vec![];
    let mut seen: HashSet<String> = HashSet::new();
    for (i, l) in code.lines().enumerate() {
        let b = l.as_bytes();
        let mut j = 0;
        while j < b.len() {
            if !(b[j].is_ascii_alphabetic() || b[j] == b'_') || (j > 0 && (b[j - 1].is_ascii_alphanumeric() || b[j - 1] == b'_')) {
                j += 1;
                continue;
            }
            let start = j;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            let w = &l[start..j];
            // Brevi's `true` and `false` are plain names to Python.
            let bool_word = matches!(w, "true" | "false") && !(start > 0 && b[start - 1] == b'.');
            if !(bool_word || l[j..].trim_start().starts_with('(')) || KEYWORDS.contains(&w) || !seen.insert(format!("{}{w}", start > 0 && b[start - 1] == b'.')) {
                continue;
            }
            let prev = l[..start].trim_end();
            if prev.ends_with("def") || prev.ends_with("class") || (brevi && start > 0 && b[start - 1] == b'.' && !brevi_only(w)) {
                continue;
            }
            let msg = if bool_word {
                format!("unknown name '{w}'")
            } else if start > 0 && b[start - 1] == b'.' {
                format!("unknown method .{w}()")
            } else {
                format!("unknown function {w}()")
            };
            diags.push(Diag::new("check", i + 1, 1, &msg));
        }
    }
    let mut out = definite(&diags, src, dir);
    out.extend(float_ranges(&code));
    out.truncate(5);
    out
}

// `0..n`: to Python the float `0.` and its attribute `n`, which is no attribute of a float. The typed parser
// reads it as Brevi's range. (`0..3` is a SyntaxError, which not_python finds.)
fn float_ranges(code: &str) -> Vec<Diag> {
    let mut out = vec![];
    for (i, l) in code.lines().enumerate() {
        for (p, _) in l.match_indices("..") {
            let name: String = l[p + 2..].chars().take_while(|c| is_word(*c)).collect();
            let number = l[..p].ends_with(|c: char| c.is_ascii_digit()) && !l[..p].rsplit(|c: char| !is_word(c)).next().unwrap_or("").starts_with(|c: char| !c.is_ascii_digit());
            if number && !name.is_empty() && !METHODS.split_whitespace().any(|x| x == name) && !maybe_unreached(code, i + 1) && !caught(code, "AttributeError") {
                out.push(Diag::new("check", i + 1, 1, "`a..b` is Brevi, not Python").fix_hint("write `range(a, b)`"));
                break;
            }
        }
    }
    out
}

// Whether the line may never run: inside (or after, on one line) a test of the Python version, the platform or
// TYPE_CHECKING, a constant false test, or any branch of such an if chain, or an except block. CPython would
// not stop there, so neither does Brevi.
fn maybe_unreached(code: &str, line: usize) -> bool {
    let lines: Vec<&str> = code.lines().collect();
    let Some(l) = lines.get(line.wrapping_sub(1)) else { return false };
    let guard = |t: &str| {
        let t = t.trim();
        let cond = t.trim_start_matches("elif ").trim_start_matches("if ").trim_start_matches("while ").trim_end_matches(':').trim();
        ["sys.version", "version_info", "sys.platform", "os.name", "platform.", "TYPE_CHECKING", "hasattr("].iter().any(|g| t.contains(g))
            || matches!(cond, "False" | "0" | "None" | "not True" | "not 1")
    };
    // Inline: `x if False else y`, `if PY2: a = unicode`.
    if [" if False", " if 0 ", " if 0:", "sys.version", "version_info", "sys.platform", "TYPE_CHECKING"].iter().any(|g| l.contains(g)) {
        return true;
    }
    let indent = |s: &str| s.len() - s.trim_start().len();
    let mut want = indent(l);
    let mut i = line - 1;
    // Walk up through the enclosing blocks; for an else/elif, also through the earlier branches of its chain.
    let mut in_chain = false;
    while i > 0 {
        i -= 1;
        let h = lines[i];
        if h.trim().is_empty() {
            continue;
        }
        let ind = indent(h);
        let t = h.trim_start();
        let opens = ind < want || (in_chain && ind == want);
        if !opens {
            continue;
        }
        if t.starts_with("except") || t.starts_with("finally") {
            return true;
        }
        if (t.starts_with("if ") || t.starts_with("elif ") || t.starts_with("while ")) && guard(t) {
            return true;
        }
        // An else/elif: the tests above it in the same chain decide whether it runs too.
        in_chain = t.starts_with("else") || t.starts_with("elif ");
        if ind == 0 && !in_chain {
            break;
        }
        want = ind;
    }
    false
}

// Whether the program catches this error anywhere (a test of it: `try: 1 + "a" except TypeError`).
fn caught(code: &str, err: &str) -> bool {
    code.contains("suppress(")
        || code.lines().map(str::trim_start).filter(|l| l.starts_with("except")).any(|l| {
            let l = l.trim_start_matches("except").trim_start_matches('*').trim();
            l.starts_with(':') || [err, "Exception", "BaseException", "LookupError"].iter().any(|e| !spots(l, e).is_empty())
        })
}

// The source with string contents and comments replaced by spaces (quotes kept), so word searches see only code.
// Lines stay where they are.
fn blank(src: &str) -> String {
    let c: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '#' {
            while i < c.len() && c[i] != '\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            let triple = i + 2 < c.len() && c[i + 1] == ch && c[i + 2] == ch;
            let q = if triple { 3 } else { 1 };
            for _ in 0..q {
                out.push(ch);
            }
            i += q;
            while i < c.len() {
                if c[i] == '\\' && i + 1 < c.len() {
                    out.push(' ');
                    out.push(if c[i + 1] == '\n' { '\n' } else { ' ' });
                    i += 2;
                    continue;
                }
                if c[i] == ch && (!triple || (i + 2 < c.len() && c[i + 1] == ch && c[i + 2] == ch)) {
                    for _ in 0..q {
                        out.push(ch);
                    }
                    i += q;
                    break;
                }
                if c[i] == '\n' && !triple {
                    break;
                }
                out.push(if c[i] == '\n' { '\n' } else { ' ' });
                i += 1;
            }
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

// Logical statements: lines joined while a bracket is open or a line ends with `\`.
fn statements(code: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut depth: i32 = 0;
    for l in code.lines() {
        for ch in l.chars() {
            match ch {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                _ => {}
            }
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        let cont = l.trim_end().ends_with('\\');
        cur.push_str(l.trim_end().trim_end_matches('\\'));
        if depth <= 0 && !cont {
            // `a = 1; b = 2` holds two statements.
            out.extend(cur.split(';').map(|s| s.to_string()));
            cur.clear();
            depth = 0;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn words(code: &str) -> HashSet<String> {
    code.split(|c: char| !is_word(c)).filter(|w| w.chars().next().map_or(false, |c| !c.is_ascii_digit())).map(String::from).collect()
}

// Where `name` stands as a whole word in s, not after a dot: (start, end) byte offsets.
fn spots(s: &str, name: &str) -> Vec<(usize, usize)> {
    let mut out = vec![];
    let mut from = 0;
    while let Some(p) = s[from..].find(name).map(|p| p + from) {
        let e = p + name.len();
        let before = s[..p].chars().last();
        let after = s[e..].chars().next();
        if !before.map_or(false, |c| is_word(c) || c == '.') && !after.map_or(false, is_word) {
            out.push((p, e));
        }
        from = e;
    }
    out
}

// A name CPython would not find: not a builtin, never bound anywhere in the file (any spot that could be an
// assignment, parameter, import, loop or `as` target counts as bound), and used on the error's line.
fn undefined(n: &str, stmts: &[String], builtins: &HashSet<&str>, line_code: &str) -> bool {
    if n.is_empty() || n.starts_with("ax_") || n.starts_with("__") || builtins.contains(n) || spots(line_code, n).is_empty() {
        return false;
    }
    const BINDERS: [&str; 15] = ["def", "class", "import", "as", "global", "nonlocal", "for", "lambda", "del", "from", "with", "except", "case", "match", "async"];
    for s in stmts {
        let t = s.trim_start();
        let first = t.split(|c: char| !is_word(c)).next().unwrap_or("");
        let found = spots(s, n);
        if found.is_empty() {
            continue;
        }
        if BINDERS.contains(&first) {
            return false;
        }
        let comp = spots(s, "for").len() + spots(s, "lambda").len() > 0;
        for (p, e) in found {
            let prev = s[..p].trim_end();
            let prev_word: String = prev.chars().rev().take_while(|c| is_word(*c)).collect::<Vec<_>>().into_iter().rev().collect();
            if BINDERS.contains(&prev_word.as_str()) {
                return false;
            }
            let rest = s[e..].trim_start();
            let aug = ["=", ":=", "+=", "-=", "*=", "/=", "//=", "%=", "**=", "&=", "|=", "^=", ">>=", "<<=", "@=", ":"];
            if aug.iter().any(|a| rest.starts_with(a)) && !rest.starts_with("==") {
                return false;
            }
            let next = rest.chars().next();
            if matches!(next, Some(',' | ')' | ']' | '}') | None) || rest.starts_with("in ") {
                // A tuple target, a parameter, a comprehension or lambda variable: bound if the statement
                // assigns after this spot or binds names in it at all.
                if comp || lone_eq(&s[e..]) {
                    return false;
                }
            }
        }
    }
    true
}

// A plain `=` (not ==, <=, >=, !=) in s.
fn lone_eq(s: &str) -> bool {
    let b = s.as_bytes();
    (0..b.len()).any(|i| b[i] == b'=' && !matches!(b.get(i + 1), Some(b'=')) && !(i > 0 && matches!(b[i - 1], b'=' | b'<' | b'>' | b'!' | b':')))
}

// A method no Python type or common module has, that the file itself never names except as `.m(` calls, in a
// file that only imports modules whose methods are known (or its own files).
fn no_such_method(m: &str, code: &str, dir: &std::path::Path, line_code: &str) -> bool {
    if m.is_empty() || m.starts_with("ax_") || m.starts_with('_') || METHODS.split_whitespace().any(|x| x == m) {
        return false;
    }
    if !line_code.contains(&format!(".{m}(")) {
        return false;
    }
    // Any other mention (a def, `self.m = f`, an attribute read) could make it real, here or in the
    // program's other files.
    let only_calls = |code: &str| {
        let mut from = 0;
        while let Some(p) = code[from..].find(m).map(|p| p + from) {
            let e = p + m.len();
            let before = code[..p].chars().last();
            let after = code[e..].chars().next();
            if !before.map_or(false, is_word) && !after.map_or(false, is_word) && !(before == Some('.') && code[e..].trim_start().starts_with('(')) {
                return false;
            }
            from = e;
        }
        true
    };
    if !only_calls(code) {
        return false;
    }
    for p in py_files(dir) {
        if std::fs::read_to_string(&p).is_ok_and(|t| !only_calls(&blank(&t))) {
            return false;
        }
    }
    // a module METHODS covers: `urllib.parse` is, `urllib.request` is not
    let full: HashSet<&str> = MODS.split_whitespace().collect();
    let known: HashSet<&str> = MODS.split_whitespace().map(|m| m.split('.').next().unwrap_or(m)).collect();
    for l in code.lines() {
        let t = l.trim();
        let mods: Vec<&str> = if let Some(r) = t.strip_prefix("import ") {
            r.split(',').map(|p| p.split_whitespace().next().unwrap_or("")).collect()
        } else if let Some(r) = t.strip_prefix("from ") {
            vec![r.split_whitespace().next().unwrap_or("")]
        } else {
            continue;
        };
        for md in mods {
            let top = md.split('.').next().unwrap_or(md);
            if top.is_empty() || top.starts_with('.') {
                continue;
            }
            if !known.contains(top) && !dir.join(format!("{top}.py")).exists() && !dir.join(top).is_dir() {
                return false;
            }
            if md != top && known.contains(top) && !full.contains(md) {
                return false;
            }
        }
    }
    true
}

// The .py files in dir and its folders (packages), three levels down, at most 500.
fn py_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    let mut todo = vec![(dir.to_path_buf(), 0)];
    while let Some((d, depth)) = todo.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() && depth < 3 {
                todo.push((p, depth + 1));
            } else if p.extension().is_some_and(|x| x == "py") && out.len() < 500 {
                out.push(p);
            }
        }
    }
    out
}

// A string literal right beside a `+` on the line (`"n=" + len(xs)`, `x + " items"`).
fn text_plus(line: &str) -> bool {
    let code = blank(line);
    let b = code.as_bytes();
    for (i, &ch) in b.iter().enumerate() {
        if ch != b'+' || b.get(i + 1) == Some(&b'=') {
            continue;
        }
        let before = code[..i].trim_end();
        let after = code[i + 1..].trim_start().trim_start_matches(|c: char| "rbfuRBFU".contains(c));
        if before.ends_with('"') || before.ends_with('\'') || after.starts_with('"') || after.starts_with('\'') {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(line: usize, msg: &str) -> Diag {
        Diag::new("check", line, 1, msg)
    }

    fn sure(src: &str, msg: &str, line: usize) -> Vec<String> {
        definite(&[d(line, msg)], src, std::path::Path::new("/nonexistent")).iter().map(|e| e.to_json()).collect()
    }

    #[test]
    fn undefined_names() {
        let src = "nums = [1, 2, 3]\ntotal = sum(nums)\nprint(totl(nums))\n";
        let got = sure(src, "unknown function totl()", 3);
        assert_eq!(got, vec![r#"{"line":3,"msg":"name 'totl' is not defined","fix":"did you mean total?"}"#]);
        // Bound somewhere: a def, a parameter, an import, a loop target, a tuple target, a lambda.
        for s in [
            "def totl(x):\n    return 1\nprint(totl(2))\n",
            "from m import totl\nprint(totl(2))\n",
            "from m import (a,\n    totl)\nprint(totl(2))\n",
            "for totl in [len]:\n    pass\nprint(totl(2))\n",
            "a, totl = 1, len\nprint(totl(2))\n",
            "f = lambda totl: totl(2)\nprint(totl(2))\n",
            "def f(a,\n      totl):\n    return totl(a)\nprint(totl(2))\n",
            "totl: int = 3\nprint(totl(2))\n",
            "with open('x') as totl:\n    pass\nprint(totl(2))\n",
            "from m import *\nprint(totl(2))\n",
            "print(divmod(7, 2))\n",
        ] {
            let n = if s.contains("divmod") { "divmod" } else { "totl" };
            let line = s.lines().count();
            assert!(sure(s, &format!("unknown function {n}()"), line).is_empty(), "{s}");
        }
        // Inside a string or comment it is not a binding.
        assert_eq!(sure("x = 'totl = 3'  # totl = 4\nprint(totl)\n", "unknown name 'totl'", 2).len(), 1);
    }

    #[test]
    fn methods() {
        let got = sure("x = 5\nprint(x.uppr())\n", "unknown method .uppr()", 2);
        assert_eq!(got, vec![r#"{"line":2,"msg":"no Python type has a method .uppr()","fix":"did you mean .upper()?"}"#]);
        // A real method of some type, a method the file defines, a third-party import: not certain.
        assert!(sure("x = 5\nprint(x.upper())\n", "unknown method .upper()", 2).is_empty());
        assert!(sure("class A:\n    def uppr(self):\n        return 1\nprint(A().uppr())\n", "unknown method .uppr()", 4).is_empty());
        assert!(sure("import numpy as np\nprint(np.zeros(3).uppr())\n", "unknown method .uppr()", 2).is_empty());
        assert!(sure("import math\nprint(math.sqrtt(2))\n", "unknown method .sqrtt()", 2).len() == 1);
    }

    #[test]
    fn scan_without_typed() {
        let src = "import sys\ndef cellval(c):\n    return c\nif (1):\n    print(cellvall(2).stripp())\n    print(len([1]), sys.argv[0].strip())\n";
        let got: Vec<String> = scan(src, std::path::Path::new("/nonexistent")).iter().map(|e| e.to_json()).collect();
        assert_eq!(got, vec![
            r#"{"line":5,"msg":"name 'cellvall' is not defined","fix":"did you mean cellval?"}"#.to_string(),
            r#"{"line":5,"msg":"no Python type has a method .stripp()","fix":"did you mean .strip()?"}"#.to_string(),
        ]);
    }

    #[test]
    fn unreached_branches() {
        let v = "import sys\nif sys.version_info[0] < 3:\n    text = unicode\nelse:\n    text = str\nprint(text(5))\n";
        assert!(sure(v, "unknown name 'unicode'", 3).is_empty());
        let v2 = "import sys\nif sys.version_info[0] >= 3:\n    text = str\nelse:\n    text = unicode\nprint(text(5))\n";
        assert!(sure(v2, "unknown name 'unicode'", 5).is_empty());
        assert!(sure("if False:\n    print(undefined_name)\n", "unknown name 'undefined_name'", 2).is_empty());
        assert!(sure("print(\"a\" + 1 if False else \"ok\")\n", "cannot add str and int", 1).is_empty());
        assert!(sure("try:\n    import x\nexcept ImportError:\n    y = fallbak()\n", "unknown function fallbak()", 4).is_empty());
        // An ordinary branch still counts.
        assert_eq!(sure("n = 3\nif n > 2:\n    print(totl(n))\n", "unknown function totl()", 3).len(), 1);
    }

    #[test]
    fn brevi_syntax() {
        let msg = |s: &str| not_python(s).map(|d| (d.line, d.msg));
        assert_eq!(msg("a = 1\nif a && 2:\n    pass\n"), Some((2, "`&&` is Brevi, not Python".to_string())));
        assert_eq!(msg("fn f(x):\n    return x\n"), Some((1, "`fn` is Brevi, not Python".to_string())));
        assert_eq!(msg("x = (1, 2).0\n"), Some((1, "`t.0` is Brevi, not Python".to_string())));
        assert_eq!(msg("t = 1\nprint(t.0)\n"), Some((2, "`t.0` is Brevi, not Python".to_string())));
        // An operator after valid Python the strict parser lacks is still found.
        assert_eq!(msg("x = b'a'\ny = 1 || 2\n"), Some((2, "`||` is Brevi, not Python".to_string())));
        // Valid Python, some of it refused by the strict parser for its own reasons.
        for s in [
            "a = 1 != 2\nb = 'x && y'  # c || d => e\nc = f\"{a!r}\"\n",
            "type X = int\n",
            "def f[T: int](x: T) -> T:\n    return x\n",
            "async def f(x: int):\n    pass\n",
            "from .. import a\nb = [...]\nc = 1..real\n",
            "x = {1} | {2}\ny = 7 // 2\nz = ~y\n",
            "def g(\n    a: bytes, b: int = 1, c: bytes = b''\n) -> None:\n    pass\n",
        ] {
            assert_eq!(msg(s), None, "{s}");
        }
    }

    #[test]
    fn float_range() {
        let got = |s: &str| float_ranges(&blank(s)).len();
        assert_eq!(got("n = 3\nfor i in 0..n:\n    pass\n"), 1);
        assert_eq!(got("a = 1..real\nb = 1..hex()\nc = x1..y\n"), 0);
        assert_eq!(got("try:\n    a = 0..n\nexcept AttributeError:\n    pass\n"), 0);
    }

    #[test]
    fn brevi_names() {
        assert_eq!(scan("x = true\nprint(x)\n", std::path::Path::new("/nonexistent")).len(), 1);
        assert_eq!(scan("def f(true=1):\n    return true\nprint(f(true=2))\n", std::path::Path::new("/nonexistent")).len(), 0);
        assert!(brevi_only("push") && brevi_only("trim") && brevi_only("is_empty"));
        assert!(!brevi_only("append") && !brevi_only("most_common") && !brevi_only("weekday") && !brevi_only("not_a_method"));
    }

    #[test]
    fn text_plus_number() {
        assert_eq!(sure("nums = [1]\nprint(\"n=\" + len(nums))\n", "cannot add str and int", 2).len(), 1);
        assert!(sure("a = f()\nprint(a + len(nums))\n", "cannot add str and int", 2).is_empty());
        // A program that tests the error itself.
        assert!(sure("try:\n    print(1 + \"a\")\nexcept TypeError as e:\n    print(e)\n", "cannot add int and str", 2).is_empty());
        assert!(sure("try:\n    totl()\nexcept:\n    pass\n", "unknown function totl()", 2).is_empty());
    }
}
