// Hugging Face tokenizers in dynamic builds, at build time: the tokenizer.json files a program loads
// (Tokenizer.from_file with a literal path, Tokenizer.from_pretrained with a literal model id, from the Hub cache
// huggingface_hub keeps, else downloaded), each one read by the same port the program runs (rt_hfcore.rs) so a
// component Brevi lacks refuses the build, and built into the program (rt_hf.rs finds them by key). Anything
// else (a path or id that is not a literal, Tokenizer(model), from_buffer) refuses the build too, and CPython's
// tokenizers runs the program.

use super::emit::visit_any;
use super::parse::{Arg, Ex, Stmt};
use std::path::{Path, PathBuf};

#[allow(dead_code)]
pub(crate) mod engine {
    use std::collections::HashMap;
    use std::rc::Rc;
    include!("rt_tokre.rs");
    include!("rt_tokuni.rs");
    include!("rt_hfuni.rs");
    include!("rt_hfcls.rs");
    include!("rt_hfnorm.rs");
    include!("rt_hfcore.rs");
}

// What a program loads: ("file:<path>" or "hub:<id>@<revision>", where to read it)
pub enum Src {
    File(String),
    Hub(String, String),
}

impl Src {
    fn key(&self) -> String {
        match self {
            Src::File(p) => format!("file:{p}"),
            Src::Hub(id, rev) => format!("hub:{id}@{rev}"),
        }
    }
}

fn is_tokenizer(e: &Ex) -> bool {
    match e {
        Ex::Name(n) => n == "Tokenizer",
        Ex::Attr(_, n) => n == "Tokenizer",
        _ => false,
    }
}

fn lit(a: Option<&Arg>) -> Option<String> {
    match a {
        Some(Arg::Pos(Ex::Str(s))) | Some(Arg::Kw(_, Ex::Str(s))) => Some(s.clone()),
        _ => None,
    }
}

// The tokenizer.json files the program's modules load, or the refusal.
pub fn check(mods: &[(&str, &[Stmt])]) -> Result<Vec<Src>, String> {
    let mut out: Vec<Src> = vec![];
    let mut bad: Option<String> = None;
    for (_, body) in mods {
        visit_any(body, &mut |e| {
            let Ex::Call(f, args) = e else { return };
            if is_tokenizer(f) {
                bad.get_or_insert(
                    "tokenizers: Tokenizer(model) (Brevi has tokenizers.json loading only)".into(),
                );
                return;
            }
            let Ex::Attr(recv, name) = &**f else { return };
            if !is_tokenizer(recv) {
                return;
            }
            let star = args
                .iter()
                .any(|a| matches!(a, Arg::Star(_) | Arg::KwStar(_)));
            match name.as_str() {
                "from_file" => match lit(args.first()) {
                    Some(p) if args.len() == 1 && !star => out.push(Src::File(p)),
                    _ => drop(bad.get_or_insert(
                        "tokenizers: Tokenizer.from_file with a path that is not a literal".into(),
                    )),
                },
                "from_pretrained" => {
                    let id = lit(args.iter().find(|a| {
                        matches!(a, Arg::Pos(_)) || matches!(a, Arg::Kw(k, _) if k == "identifier")
                    }));
                    let rev = match args.iter().enumerate().find(|(i, a)| {
                        *i == 1 && matches!(a, Arg::Pos(_))
                            || matches!(a, Arg::Kw(k, _) if k == "revision")
                    }) {
                        None => Some("main".to_string()),
                        Some((_, a)) => lit(Some(a)),
                    };
                    match (id, rev) {
                        (Some(id), Some(rev)) if !star => out.push(Src::Hub(id, rev)),
                        _ => drop(bad.get_or_insert("tokenizers: Tokenizer.from_pretrained with a model id or revision that is not a literal".into())),
                    }
                }
                "from_str" => {}
                m => drop(bad.get_or_insert(format!("tokenizers: Tokenizer.{m}"))),
            }
        });
    }
    match bad {
        Some(b) => Err(b),
        None => Ok(out),
    }
}

// ---------- the files ----------

fn hub_cache() -> PathBuf {
    if let Ok(d) = std::env::var("HF_HUB_CACHE") {
        return PathBuf::from(d);
    }
    if let Ok(d) = std::env::var("HF_HOME") {
        return PathBuf::from(d).join("hub");
    }
    let base = std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cache")
        });
    base.join("huggingface").join("hub")
}

// tokenizer.json of a Hub model: from huggingface_hub's cache, else downloaded (into Brevi's cache)
fn hub_file(cache: &Path, id: &str, rev: &str) -> Result<PathBuf, String> {
    let repo = hub_cache().join(format!("models--{}", id.replace('/', "--")));
    let sha = std::fs::read_to_string(repo.join("refs").join(rev))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| rev.to_string());
    let p = repo.join("snapshots").join(&sha).join("tokenizer.json");
    if p.is_file() {
        return Ok(p);
    }
    let safe: String = format!("{id}@{rev}")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-_.@".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let dst = cache.join("hf").join(format!("{safe}.json"));
    if dst.is_file() {
        return Ok(dst);
    }
    std::fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
    let tmp = dst.with_extension(format!("tmp{}", std::process::id()));
    let url = format!("https://huggingface.co/{id}/resolve/{rev}/tokenizer.json");
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-fsSL", "--retry", "2", "--max-time", "300", "-o"])
        .arg(&tmp);
    if let Ok(t) = std::env::var("HF_TOKEN") {
        cmd.arg("-H").arg(format!("Authorization: Bearer {t}"));
    }
    let ok = cmd.arg(&url).status().is_ok_and(|s| s.success());
    if !ok {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("tokenizers: could not get {url}"));
    }
    std::fs::rename(&tmp, &dst).map_err(|e| e.to_string())?;
    Ok(dst)
}

// Each file checked (read by Brevi's port, as the program will) and copied into Brevi's cache: (key, path)
pub fn files(cache: &Path, srcs: &[Src]) -> Result<Vec<(String, PathBuf)>, String> {
    let mut out: Vec<(String, PathBuf)> = vec![];
    for s in srcs {
        let key = s.key();
        if out.iter().any(|x| x.0 == key) {
            continue;
        }
        let path = match s {
            Src::File(p) => PathBuf::from(p),
            Src::Hub(id, rev) => hub_file(cache, id, rev)?,
        };
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("tokenizers: {}: {e}", path.display()))?;
        if let Err(e) = engine::hf_load(&text) {
            return Err(match e {
                engine::HfErr::Py(m) => {
                    format!("tokenizers: {} does not load ({m})", path.display())
                }
                engine::HfErr::Lacks(m) => {
                    format!("tokenizers: {} uses {m}, which Brevi lacks", path.display())
                }
            });
        }
        // a copy named by content, so a later change to the file rebuilds the program
        let h = text.bytes().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ b as u64).wrapping_mul(0x100000001b3)
        });
        let dst = cache.join("hf").join(format!("{h:016x}.json"));
        if !dst.is_file() {
            std::fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
            let t = dst.with_extension(format!("tmp{}", std::process::id()));
            std::fs::write(&t, &text).map_err(|e| e.to_string())?;
            std::fs::rename(&t, &dst).map_err(|e| e.to_string())?;
        }
        out.push((key, dst));
    }
    Ok(out)
}

// The program's main, with the files built in.
pub fn embed(rust: String, files: &[(String, PathBuf)]) -> String {
    let items: Vec<String> = files
        .iter()
        .map(|(k, p)| format!("({k:?}, include_str!({:?}))", p.to_string_lossy()))
        .collect();
    rust.replacen(
        "fn main() { ",
        &format!(
            "fn main() {{ HF_DATA.set(vec![{}]).ok(); ",
            items.join(", ")
        ),
        1,
    )
}
