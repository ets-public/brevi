// Using code outside Brevi: Rust crates and Python.
//
//   use sha2 "0.10"                 # a crate (version and features optional: use reqwest "0.12" "blocking json")
//   rust fn hex(s: str) -> str:     # a function written in Rust, called like any Brevi function
//     use sha2::Digest;
//     format!("{:x}", sha2::Sha256::digest(s.as_bytes()))
//   py fn mean(xs: [f64]) -> f64:   # a function written in Python; values cross as JSON
//     import statistics
//     return statistics.mean(xs)
//   rust:                           # Rust items shared by the rust fns (use lines, helpers, impls)
//   py:                             # Python code run once at start (imports, helpers)
//
// The source is rewritten before lexing: each foreign function becomes `fn NAME(PARAMS) -> RET = __rust__(K)`
// on its own line, and its body lines become blank, so line numbers stay the same.
use crate::ast::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Dep {
    pub name: String,
    pub version: String,
    pub features: Vec<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Foreign {
    pub deps: Vec<Dep>,
    pub rust_fns: Vec<String>,         // bodies, indexed by the K in __rust__(K)
    pub py_fns: Vec<(String, String)>, // (param names joined by ", ", body)
    pub rust_top: String,              // `rust:` blocks
    pub py_top: String,                // `py:` blocks
    pub rust_lines: Vec<(usize, usize)>, // first and last source line of each rust fn and rust: block
    pub rust_starts: Vec<usize>,       // the source line of each rust fn body's first line
}

fn indent_of(l: &str) -> usize {
    l.len() - l.trim_start().len()
}

// The lines below `start` that belong to a block opened at column 0, dedented.
fn take_block(lines: &[&str], start: usize) -> (usize, String) {
    let mut end = start;
    while end < lines.len() && (lines[end].trim().is_empty() || indent_of(lines[end]) > 0) {
        end += 1;
    }
    while end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    let body = &lines[start..end];
    let min = body.iter().filter(|l| !l.trim().is_empty()).map(|l| indent_of(l)).min().unwrap_or(0);
    let text: Vec<&str> = body.iter().map(|l| if l.len() >= min { &l[min..] } else { l.trim_start() }).collect();
    (end, text.join("\n"))
}

// `use NAME`, `use NAME "1.0"`, `use NAME "1.0" "feat1 feat2"`.
fn parse_use(l: &str, line: usize) -> Option<Dep> {
    let rest = l.strip_prefix("use ")?;
    let rest = match rest.find(" #") {
        Some(i) => &rest[..i],
        None => rest,
    }
    .trim();
    let (name, mut tail) = rest.split_once(char::is_whitespace).map_or((rest, ""), |(a, b)| (a, b.trim()));
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') || !name.chars().next()?.is_ascii_alphabetic() {
        return None;
    }
    let mut strs = vec![];
    while let Some(t) = tail.strip_prefix('"') {
        let e = t.find('"')?;
        strs.push(t[..e].to_string());
        tail = t[e + 1..].trim_start();
    }
    if !tail.is_empty() || strs.len() > 2 {
        return None;
    }
    let version = strs.first().cloned().filter(|v| !v.is_empty()).unwrap_or_else(|| "*".into());
    let features = strs.get(1).map(|f| f.split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()).map(String::from).collect()).unwrap_or_default();
    Some(Dep { name: name.to_string(), version, features, line })
}

// Splits `rust fn sig:` / `rust fn sig = expr` into (sig, Some(one-line body)) where sig ends after the return type.
fn split_sig(rest: &str) -> Option<(String, Option<String>)> {
    let open = rest.find('(')?;
    let mut depth = 0;
    let mut close = None;
    for (i, c) in rest.char_indices().skip(open) {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    let after = &rest[close + 1..];
    if let Some(eq) = after.find('=').filter(|&i| !after[i..].starts_with("=>")) {
        let body = after[eq + 1..].trim().to_string();
        return Some((format!("{}{}", &rest[..=close], after[..eq].trim_end()), Some(body)));
    }
    let t = after.trim_end();
    let t = t.strip_suffix(':')?;
    Some((format!("{}{}", &rest[..=close], t.trim_end()), None))
}

pub fn split(src: &str) -> (String, Foreign) {
    let lines: Vec<&str> = src.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut f = Foreign::default();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        // `use "lib.bv"` was handled by modules::expand.
        if l.starts_with("use \"") {
            out.push(String::new());
            i += 1;
            continue;
        }
        if let Some(d) = parse_use(l.trim_end(), i + 1) {
            f.deps.push(d);
            out.push(String::new());
            i += 1;
            continue;
        }
        let t = l.trim_end();
        if t == "rust:" || t == "py:" {
            let (end, body) = take_block(&lines, i + 1);
            if t == "rust:" {
                f.rust_lines.push((i + 1, end));
                f.rust_top.push_str(&marked(&body, i + 2));
                f.rust_top.push('\n');
                for _ in i..end {
                    out.push(String::new());
                }
                i = end;
                continue;
            }
            let top = &mut f.py_top;
            top.push_str(&body);
            top.push('\n');
            for _ in i..end {
                out.push(String::new());
            }
            i = end;
            continue;
        }
        let lang = if l.starts_with("rust fn ") {
            "rust"
        } else if l.starts_with("py fn ") {
            "py"
        } else {
            out.push(l.to_string());
            i += 1;
            continue;
        };
        let rest = &l[lang.len() + 1..];
        let Some((sig, inline)) = split_sig(rest) else {
            out.push(l.to_string());
            i += 1;
            continue;
        };
        let (end, body) = match inline {
            Some(b) => (i + 1, if lang == "py" { format!("return {b}") } else { b }),
            None => take_block(&lines, i + 1),
        };
        let mut sig = sig;
        if !sig.contains("->") {
            sig.push_str(" -> ()");
        }
        if lang == "py" && !sig.trim_end().ends_with('!') {
            sig.push_str(" !");
        }
        let k = if lang == "rust" {
            f.rust_lines.push((i + 1, end.max(i + 1)));
            f.rust_starts.push(if end == i + 1 { i + 1 } else { i + 2 });
            f.rust_fns.push(body);
            f.rust_fns.len() - 1
        } else {
            let open = sig.find('(').unwrap_or(0);
            let close = sig.rfind(')').unwrap_or(open);
            let names: Vec<String> = split_params(&sig[open + 1..close]);
            f.py_fns.push((names.join(", "), body));
            f.py_fns.len() - 1
        };
        out.push(format!("{sig} = __{lang}__({k})"));
        for _ in i + 1..end {
            out.push(String::new());
        }
        i = end;
    }
    let mut s = out.join("\n");
    if src.ends_with('\n') {
        s.push('\n');
    }
    (s, f)
}

// Parameter names from `a: int, mut b: [str], c: {str: int}`.
fn split_params(s: &str) -> Vec<String> {
    let mut parts = vec![];
    let mut depth = 0;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    parts.push(cur);
    parts
        .iter()
        .filter_map(|p| p.split(':').next())
        .map(|n| n.trim().trim_start_matches("mut ").trim().to_string())
        .filter(|n| !n.is_empty())
        .collect()
}

// Rust code with a line marker on each line, so rustc errors point at the Brevi source line.
pub fn marked(code: &str, first: usize) -> String {
    code.lines().enumerate().map(|(k, l)| format!("/*@L{}*/{l}", first + k)).collect::<Vec<_>>().join("\n")
}

// Puts each foreign body into its marker call: `__rust__(K)` becomes `__rust__("code")`.
pub fn attach(items: &mut [Item], f: &Foreign) {
    for it in items.iter_mut() {
        if let Item::Fn { body, .. } = it {
            if let Expr::Call(name, args, _) = body {
                if (name == "__rust__" || name == "__py__") && args.len() == 1 {
                    if let Expr::Int(k) = args[0].1 {
                        let code = if name == "__rust__" { f.rust_fns.get(k as usize).map(|c| marked(c, f.rust_starts[k as usize])) } else { f.py_fns.get(k as usize).map(|p| p.1.clone()) };
                        args[0].1 = Expr::Str(vec![StrPart::Lit(code.unwrap_or_default())]);
                    }
                }
            }
        }
    }
}

// The foreign body of a function made by `rust fn` or `py fn`.
pub fn foreign_body(body: &Expr) -> Option<(&'static str, &str)> {
    if let Expr::Call(name, args, _) = body {
        let lang = match name.as_str() {
            "__rust__" => "rust",
            "__py__" => "py",
            _ => return None,
        };
        if let [(_, Expr::Str(parts))] = args.as_slice() {
            if let [StrPart::Lit(code)] = parts.as_slice() {
                return Some((lang, code));
            }
            if parts.is_empty() {
                return Some((lang, ""));
            }
        }
    }
    None
}

const NOT_CRATES: &[&str] = &[
    "std", "core", "alloc", "crate", "self", "super", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
    "f32", "f64", "bool", "char", "str", "proc_macro",
];

// Crates that rust code names by path (`sha2::Sha256`, `use rand::Rng`) and no `use` line declares: added at any version.
pub fn all_deps(f: &Foreign) -> Vec<Dep> {
    let mut deps = f.deps.clone();
    let code: String = f.rust_fns.iter().map(|s| s.as_str()).chain([f.rust_top.as_str()]).collect::<Vec<_>>().join("\n");
    let b = code.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        // Skip string literals and comments.
        if c == b'"' {
            i += 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c.is_ascii_lowercase() && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] == b':' || b[i - 1] == b'\'' || b[i - 1] == b'.')) {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let w = &code[s..i];
            let local_mod = code.contains(&format!("mod {w} ")) || code.contains(&format!("mod {w}{{"));
            if code[i..].starts_with("::") && !code[i..].starts_with("::<") && !local_mod && !NOT_CRATES.contains(&w) && !w.starts_with("ax_") && !deps.iter().any(|d| d.name.replace('-', "_") == w) {
                deps.push(Dep { name: w.to_string(), version: "*".into(), features: vec![], line: 0 });
            }
            continue;
        }
        i += 1;
    }
    deps
}

pub fn cargo_toml(name: &str, deps: &[Dep]) -> String {
    let mut s = format!(
        "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[[bin]]\nname = \"{name}\"\npath = \"main.rs\"\ntest = true\n\n[profile.dev]\noverflow-checks = true\ndebug = false\n\n[profile.release]\noverflow-checks = true\n\n[dependencies]\n"
    );
    for d in deps {
        s.push_str(&format!("{} = {{ version = \"{}\"", d.name, d.version));
        // A `-default` feature (not a valid cargo feature name) means default-features = false.
        if d.features.iter().any(|f| f == "-default") {
            s.push_str(", default-features = false");
        }
        let features: Vec<&String> = d.features.iter().filter(|f| *f != "-default").collect();
        if !features.is_empty() {
            let fs: Vec<String> = features.iter().map(|f| format!("\"{f}\"")).collect();
            s.push_str(&format!(", features = [{}]", fs.join(", ")));
        }
        s.push_str(" }\n");
    }
    s
}

// Rust for one foreign function. Brevi values arrive as Rust values (str is String, [T] is Vec<T>, `mut` params are &mut).
pub fn rust_fn(name: &str, params: &[Param], ret: &Ty, fallible: bool, lang: &str, code: &str, line: usize, py_index: usize) -> String {
    use crate::emit::rust_ty;
    let ps: Vec<String> = params.iter().map(|p| if p.is_mut { format!("{}: &mut {}", p.name, rust_ty(&p.ty)) } else { format!("mut {}: {}", p.name, rust_ty(&p.ty)) }).collect();
    let names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
    let rt = rust_ty(ret);
    if lang == "py" {
        let args: Vec<String> = params.iter().map(|p| format!("AxToJson::ax_json(&{}{})", if p.is_mut { "*" } else { "" }, p.name)).collect();
        return format!(
            "/*@L{line}*/fn {name}({}) -> R<{rt}> {{\n    AX_LINE.store({line}, std::sync::atomic::Ordering::Relaxed);\n    let v = ax_py({py_index}, vec![{}])?;\n    AxFromJson::ax_from(&v).map_err(|m| AxErr(format!(\"{name}: python returned {{}}, {{}}\", v.show(), m)))\n}}\n",
            ps.join(", "),
            args.join(", ")
        );
    }
    let code = code.replace('\n', "\n        ");
    let code = code.trim_start();
    if fallible {
        let inner: Vec<String> = params.iter().map(|p| if p.is_mut { format!("{}: &mut {}", p.name, rust_ty(&p.ty)) } else { format!("mut {}: {}", p.name, rust_ty(&p.ty)) }).collect();
        format!(
            "/*@L{line}*/fn {name}({}) -> R<{rt}> {{\n    #[allow(unused_mut, unused_imports)]\n    fn ax_inner({}) -> Result<{rt}, Box<dyn std::error::Error>> {{\n        {code}\n    }}\n/*@L{line}*/    ax_inner({}).map_err(|e| AxErr(e.to_string()))\n}}\n",
            ps.join(", "),
            inner.join(", "),
            names.join(", ")
        )
    } else {
        let r = if matches!(ret, Ty::Unit) { String::new() } else { format!(" -> {rt}") };
        format!("/*@L{line}*/#[allow(unused_mut, unused_imports)]\nfn {name}({}){r} {{\n        {code}\n}}\n", ps.join(", "))
    }
}

// The Python program that runs py fns: one process for the whole run, one JSON line per call.
pub fn py_driver(f: &Foreign, names: &[String]) -> String {
    let mut s = String::from("import sys, json\n_ax_out = sys.stdout\nsys.stdout = sys.stderr\n");
    s.push_str(&f.py_top);
    s.push('\n');
    for (k, (params, body)) in f.py_fns.iter().enumerate() {
        let name = names.get(k).cloned().unwrap_or_else(|| format!("f{k}"));
        s.push_str(&format!("def {name}({params}):\n"));
        let b = if body.trim().is_empty() { "pass".to_string() } else { body.clone() };
        for l in b.lines() {
            s.push_str("    ");
            s.push_str(l);
            s.push('\n');
        }
    }
    let list: Vec<String> = (0..f.py_fns.len()).map(|k| names.get(k).cloned().unwrap_or_else(|| format!("f{k}"))).collect();
    s.push_str(&format!("_ax_fns = [{}]\n", list.join(", ")));
    s.push_str(
        "def _ax_conv(o):\n    if hasattr(o, 'tolist'): return o.tolist()\n    if isinstance(o, (set, frozenset)): return sorted(o)\n    if hasattr(o, 'item'): return o.item()\n    if hasattr(o, '__dict__'): return o.__dict__\n    return str(o)\n\
for _ax_l in sys.stdin:\n    _ax_r = json.loads(_ax_l)\n    try:\n        _ax_v = {'ok': _ax_fns[_ax_r[0]](*_ax_r[1])}\n    except Exception as _ax_e:\n        _ax_v = {'err': type(_ax_e).__name__ + ': ' + str(_ax_e)}\n    _ax_out.write(json.dumps(_ax_v, default=_ax_conv) + '\\n')\n    _ax_out.flush()\n",
    );
    s
}

// Rust support code for py fns: the process, and conversion from Json for each user type.
pub fn py_support(driver: &str, items: &[Item]) -> String {
    let mut s = format!("/*@L0*/const AX_PY_SRC: &str = {driver:?};\n");
    s.push_str(PY_RUNTIME);
    for it in items {
        match it {
            Item::Type { name, fields, .. } => {
                let to: Vec<String> = fields.iter().map(|f| format!("m.insert({:?}.to_string(), AxToJson::ax_json(&self.{}));", f.name, f.name)).collect();
                let from: Vec<String> = fields.iter().map(|f| format!("{}: AxFromJson::ax_from(o.get(&{:?}.to_string()).unwrap_or(&Json::Null)).map_err(|e| format!(\"field {}: {{e}}\"))?,", f.name, f.name, f.name)).collect();
                s.push_str(&format!(
                    "impl AxToJson for {name} {{ fn ax_json(&self) -> Json {{ let mut m = AxM::ordered(); {} Json::Obj {{ v: m }} }} }}\nimpl AxFromJson for {name} {{ fn ax_from(j: &Json) -> Result<Self, String> {{ match j {{ Json::Obj {{ v: o }} => Ok({name} {{ {} }}), _ => Err(\"expected an object\".into()) }} }} }}\n",
                    to.join(" "),
                    from.join(" ")
                ));
            }
            _ => {}
        }
    }
    s
}

const PY_RUNTIME: &str = r#"
pub trait AxFromJson: Sized {
    fn ax_from(j: &Json) -> Result<Self, String>;
}
impl AxFromJson for Json {
    fn ax_from(j: &Json) -> Result<Self, String> { Ok(j.clone()) }
}
impl AxFromJson for () {
    fn ax_from(_: &Json) -> Result<Self, String> { Ok(()) }
}
impl AxFromJson for i64 {
    fn ax_from(j: &Json) -> Result<Self, String> {
        match j { Json::Num { v, .. } if v.fract() == 0.0 => Ok(*v as i64), Json::Bool { v } => Ok(*v as i64), _ => Err("expected int".into()) }
    }
}
impl AxFromJson for f64 {
    fn ax_from(j: &Json) -> Result<Self, String> {
        match j { Json::Num { v, .. } => Ok(*v), _ => Err("expected f64".into()) }
    }
}
impl AxFromJson for bool {
    fn ax_from(j: &Json) -> Result<Self, String> {
        match j { Json::Bool { v } => Ok(*v), _ => Err("expected bool".into()) }
    }
}
impl AxFromJson for String {
    fn ax_from(j: &Json) -> Result<Self, String> {
        match j { Json::Str { v } => Ok(v.clone()), Json::Null => Err("expected str".into()), o => Ok(o.show()) }
    }
}
impl AxFromJson for Dec {
    fn ax_from(j: &Json) -> Result<Self, String> {
        match j { Json::Num { v, .. } => v.to_string().ax_dec().map_err(|e| e.0), Json::Str { v } => v.clone().ax_dec().map_err(|e| e.0), _ => Err("expected dec".into()) }
    }
}
impl<T: AxFromJson> AxFromJson for Vec<T> {
    fn ax_from(j: &Json) -> Result<Self, String> {
        match j { Json::Arr { v } => v.iter().map(T::ax_from).collect(), _ => Err("expected a list".into()) }
    }
}
impl<K: AxFromJsonKey + Hash + Eq + Ord + Clone, V: AxFromJson> AxFromJson for AxM<K, V> {
    fn ax_from(j: &Json) -> Result<Self, String> {
        match j {
            Json::Obj { v } => {
                let mut m = AxM::new();
                for (k, x) in v.iter() { m.insert(K::ax_key(k)?, V::ax_from(x)?); }
                Ok(m)
            }
            _ => Err("expected a map".into()),
        }
    }
}
pub trait AxFromJsonKey: Sized { fn ax_key(s: &str) -> Result<Self, String>; }
impl AxFromJsonKey for String { fn ax_key(s: &str) -> Result<Self, String> { Ok(s.to_string()) } }
impl AxFromJsonKey for i64 { fn ax_key(s: &str) -> Result<Self, String> { s.parse().map_err(|_| format!("key {s} is not an int")) } }
macro_rules! ax_tuple_json {
    ($($t:ident $i:tt),+) => {
        impl<$($t: AxFromJson),+> AxFromJson for ($($t,)+) {
            fn ax_from(j: &Json) -> Result<Self, String> {
                match j { Json::Arr { v } => Ok(($($t::ax_from(v.get($i).unwrap_or(&Json::Null))?,)+)), _ => Err("expected a list for a tuple".into()) }
            }
        }
    };
}
ax_tuple_json!(A 0, B 1);
ax_tuple_json!(A 0, B 1, C 2);
ax_tuple_json!(A 0, B 1, C 2, D 3);
impl AxToJson for () { fn ax_json(&self) -> Json { Json::Null } }

static AX_PY: std::sync::Mutex<Option<(std::process::ChildStdin, std::io::BufReader<std::process::ChildStdout>)>> = std::sync::Mutex::new(None);
pub fn ax_py(f: usize, args: Vec<Json>) -> R<Json> {
    use std::io::{BufRead, Write};
    let mut g = AX_PY.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_none() {
        let exe = std::env::var("BREVI_PYTHON").unwrap_or_else(|_| if cfg!(windows) { "python".to_string() } else { "python3".to_string() });
        let mut c = std::process::Command::new(&exe)
            .args(["-u", "-c", AX_PY_SRC])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| AxErr(format!("cannot start {exe}: {e}")))?;
        let i = c.stdin.take().unwrap();
        let o = std::io::BufReader::new(c.stdout.take().unwrap());
        *g = Some((i, o));
    }
    let (i, o) = g.as_mut().unwrap();
    let req = format!("[{f},{}]\n", Json::Arr { v: args }.show());
    if i.write_all(req.as_bytes()).is_err() {
        return Err(AxErr("python stopped (its error is above)".into()));
    }
    let mut line = String::new();
    if o.read_line(&mut line).unwrap_or(0) == 0 {
        return Err(AxErr("python stopped (its error is above)".into()));
    }
    match json(line)? {
        Json::Obj { v } => match (v.get(&"ok".to_string()), v.get(&"err".to_string())) {
            (Some(x), _) => Ok(x.clone()),
            (_, Some(Json::Str { v: m })) => Err(AxErr(m.clone())),
            _ => Err(AxErr("bad reply from python".into())),
        },
        _ => Err(AxErr("bad reply from python".into())),
    }
}
"#;
