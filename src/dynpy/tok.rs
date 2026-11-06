// tiktoken in dynamic builds, at build time: which of OpenAI's vocabularies a program needs, getting them (from
// tiktoken's own cache, else downloaded the way tiktoken downloads them, with its hashes checked) and building
// them into the program (rt_tok.rs reads them), and the check that every split pattern the program gives
// tiktoken.Encoding is one Brevi's regex engine has (rt_tokre.rs). Anything else refuses the build, and CPython's
// tiktoken runs the program.

use super::emit::visit_any;
use super::parse::{Arg, Ex, Stmt};
use std::path::{Path, PathBuf};

#[allow(dead_code)]
mod engine {
    include!("rt_tokre.rs");
    include!("rt_tokuni.rs");
}

// the files of OpenAI's encodings: (file, sha256 tiktoken checks)
const FILES: &[(&str, &str)] = &[
    ("r50k_base", "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930"),
    ("p50k_base", "94b5ca7dff4d00767bc256fdd1b27e5b17361d7b8a5f968547f9f23eb70d2069"),
    ("cl100k_base", "223921b76ee99bde995b7ff738513eef100fb51d18c93597a113bcffe865b2a7"),
    ("o200k_base", "446a9538cb6c348e3516120d7c08b09f57c36495e2acfffe59a5bf8b0cfb1a2d"),
];

fn file_of(encoding: &str) -> Option<&'static str> {
    Some(match encoding {
        "gpt2" | "r50k_base" => "r50k_base",
        "p50k_base" | "p50k_edit" => "p50k_base",
        "cl100k_base" => "cl100k_base",
        "o200k_base" | "o200k_harmony" => "o200k_base",
        _ => return None,
    })
}

// encoding_name_for_model, from the tables of Brevi's copy (py/tiktoken.py)
fn model_encoding(model: &str) -> Option<String> {
    let src = include_str!("py/tiktoken.py");
    let table = |name: &str| -> Vec<(String, String)> {
        let at = src.find(&format!("\n{name} = {{")).unwrap();
        let body = &src[at..];
        let body = &body[..body.find("\n}").unwrap()];
        body.lines()
            .filter_map(|l| {
                let l = l.trim();
                let (k, v) = l.split_once(": ")?;
                Some((k.trim_matches('"').to_string(), v.trim_end_matches(',').trim_matches('"').to_string()))
            })
            .collect()
    };
    if let Some((_, e)) = table("MODEL_TO_ENCODING").into_iter().find(|(k, _)| k == model) {
        return Some(e);
    }
    table("MODEL_PREFIX_TO_ENCODING").into_iter().find(|(k, _)| model.starts_with(k.as_str())).map(|x| x.1)
}

fn call_name(f: &Ex) -> Option<&str> {
    match f {
        Ex::Name(n) => Some(n),
        Ex::Attr(_, n) => Some(n),
        _ => None,
    }
}

// The vocabulary files the program's modules need (every one when a name is not a literal), or the refusal:
// a split pattern Brevi lacks.
pub fn check(mods: &[(&str, &[Stmt])]) -> Result<Vec<&'static str>, String> {
    let mut files: Vec<&'static str> = vec![];
    let mut all = false;
    let mut bad: Option<String> = None;
    for (_, body) in mods {
        visit_any(body, &mut |e| {
            let Ex::Call(f, args) = e else { return };
            let name = call_name(f).unwrap_or("");
            for a in args {
                match a {
                    Arg::Kw(k, v) if k == "pat_str" => match v {
                        Ex::Str(p) => {
                            if let Err(m) = engine::tk_compile(p) {
                                bad.get_or_insert(format!("tiktoken: the split pattern {p:?} ({m}) is not in Brevi's regex engine"));
                            }
                        }
                        Ex::Attr(_, a) if a == "_pat_str" => {}
                        _ => drop(bad.get_or_insert("tiktoken: a split pattern Brevi cannot see at build time".into())),
                    },
                    Arg::KwStar(_) | Arg::Star(_) if name == "Encoding" => drop(bad.get_or_insert("tiktoken: Encoding(*args or **kwargs)".into())),
                    _ => {}
                }
            }
            if matches!(name, "get_encoding" | "encoding_for_model") {
                match args.first() {
                    Some(Arg::Pos(Ex::Str(s))) | Some(Arg::Kw(_, Ex::Str(s))) if args.len() == 1 => {
                        let enc = if name == "get_encoding" { Some(s.clone()) } else { model_encoding(s) };
                        if let Some(f) = enc.as_deref().and_then(file_of) {
                            if !files.contains(&f) {
                                files.push(f);
                            }
                        }
                    }
                    _ => all = true,
                }
            }
        });
    }
    if let Some(b) = bad {
        return Err(b);
    }
    if all {
        files = FILES.iter().map(|x| x.0).collect();
    }
    Ok(files)
}

// ---------- the files ----------

fn tiktoken_cache() -> Option<PathBuf> {
    let d = std::env::var("TIKTOKEN_CACHE_DIR").or_else(|_| std::env::var("DATA_GYM_CACHE_DIR"));
    match d {
        Ok(d) if d.is_empty() => None,
        Ok(d) => Some(PathBuf::from(d)),
        Err(_) => Some(std::env::temp_dir().join("data-gym-cache")),
    }
}

fn url(file: &str) -> String {
    format!("https://openaipublic.blob.core.windows.net/encodings/{file}.tiktoken")
}

// The .tiktoken file: from tiktoken's cache when it is there and its hash is right, else downloaded (with curl)
// and put in that cache as tiktoken would.
fn fetch(file: &str, sha: &str) -> Result<Vec<u8>, String> {
    let key = hex(&sha1(url(file).as_bytes()));
    let cache = tiktoken_cache();
    if let Some(c) = &cache {
        if let Ok(d) = std::fs::read(c.join(&key)) {
            if hex(&sha256(&d)) == sha {
                return Ok(d);
            }
        }
    }
    let tmp = std::env::temp_dir().join(format!("brevi-tok-{}-{file}", std::process::id()));
    let ok = std::process::Command::new("curl")
        .args(["-fsSL", "--retry", "2", "--max-time", "120", "-o"])
        .arg(&tmp)
        .arg(url(file))
        .status()
        .is_ok_and(|s| s.success());
    let d = std::fs::read(&tmp).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp);
    if !ok || hex(&sha256(&d)) != sha {
        return Err(format!("tiktoken: could not get the {file} vocabulary ({})", url(file)));
    }
    if let Some(c) = &cache {
        if std::fs::create_dir_all(c).is_ok() {
            let t = c.join(format!("{key}.brevi{}.tmp", std::process::id()));
            if std::fs::write(&t, &d).is_ok() {
                let _ = std::fs::rename(&t, c.join(&key));
            }
        }
    }
    Ok(d)
}

fn b64(s: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut n) = (0u32, 0);
    for &c in s {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((acc >> n) as u8);
        }
    }
    Some(out)
}

// "token rank" lines into rt_tok.rs's blob: "BTK1", count, (length, bytes)..., gap count, (index, rank)...
fn blob(text: &[u8]) -> Result<Vec<u8>, String> {
    let mut ents: Vec<(Vec<u8>, u32)> = vec![];
    for l in text.split(|&c| c == b'\n').filter(|l| !l.is_empty()) {
        let mut w = l.split(|&c| c == b' ');
        let tok = w.next().and_then(b64);
        let rank = w.next().and_then(|r| std::str::from_utf8(r).ok()?.trim().parse::<u32>().ok());
        match (tok, rank) {
            (Some(t), Some(r)) if t.len() < 256 => ents.push((t, r)),
            _ => return Err("tiktoken: a vocabulary line Brevi cannot read".into()),
        }
    }
    let mut out = b"BTK1".to_vec();
    out.extend((ents.len() as u32).to_le_bytes());
    let mut gaps = vec![];
    for (i, (t, r)) in ents.iter().enumerate() {
        if i == 0 && *r != 0 || i > 0 && *r != ents[i - 1].1.wrapping_add(1) {
            gaps.push((i as u32, *r));
        }
        out.push(t.len() as u8);
        out.extend(t);
    }
    out.extend((gaps.len() as u32).to_le_bytes());
    for (i, r) in gaps {
        out.extend(i.to_le_bytes());
        out.extend(r.to_le_bytes());
    }
    Ok(out)
}

// Each file as a blob in Brevi's cache: (file, path of the blob)
pub fn blobs(cache: &Path, files: &[&'static str]) -> Result<Vec<(&'static str, PathBuf)>, String> {
    let mut out = vec![];
    for f in files {
        let sha = FILES.iter().find(|x| x.0 == *f).unwrap().1;
        let p = cache.join("tok").join(format!("{f}-{}.btk", &sha[..16]));
        if !p.exists() {
            let b = blob(&fetch(f, sha)?)?;
            std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
            let t = p.with_extension(format!("tmp{}", std::process::id()));
            std::fs::write(&t, b).map_err(|e| e.to_string())?;
            std::fs::rename(&t, &p).map_err(|e| e.to_string())?;
        }
        out.push((*f, p));
    }
    Ok(out)
}

// The program's main, with the blobs built in.
pub fn embed(rust: String, blobs: &[(&str, PathBuf)]) -> String {
    let items: Vec<String> = blobs.iter().map(|(f, p)| format!("({f:?}, include_bytes!({:?}) as &[u8])", p.to_string_lossy())).collect();
    rust.replacen("fn main() { ", &format!("fn main() {{ TOK_DATA.set(vec![{}]).ok(); ", items.join(", ")), 1)
}

// ---------- sha1 and sha256 ----------

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn pad(msg: &[u8]) -> Vec<u8> {
    let mut m = msg.to_vec();
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend(((msg.len() as u64) * 8).to_be_bytes());
    m
}

fn sha1(msg: &[u8]) -> Vec<u8> {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    for ch in pad(msg).chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(ch[4 * i..4 * i + 4].try_into().unwrap());
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().flat_map(|x| x.to_be_bytes()).collect()
}

fn sha256(msg: &[u8]) -> Vec<u8> {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    for ch in pad(msg).chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(ch[4 * i..4 * i + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for (x, y) in h.iter_mut().zip(v) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().flat_map(|x| x.to_be_bytes()).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn hashes() {
        assert_eq!(super::hex(&super::sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(super::hex(&super::sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
    #[test]
    fn models() {
        assert_eq!(super::model_encoding("gpt-4o-mini").as_deref(), Some("o200k_base"));
        assert_eq!(super::model_encoding("gpt-4").as_deref(), Some("cl100k_base"));
        assert_eq!(super::model_encoding("text-davinci-003").as_deref(), Some("p50k_base"));
        assert_eq!(super::model_encoding("nope"), None);
    }
}
