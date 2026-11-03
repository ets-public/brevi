// `brevi mcp`: a Model Context Protocol server on stdin/stdout (JSON-RPC, one message per line).
// Tools: guide, check, run, test, patch, edit, show. The server keeps the program: send `code` once, then
// change it with patch (whole defs by name) or edit (a range of lines), so a retry costs only the change.
// Code is plain Python (kept as main.py, built like `brevi run main.py`) unless it is Brevi (`fn` items, or a
// file name ending .bv).
use crate::diag::esc;
use crate::json::{parse, J};
use std::io::{BufRead, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn to_json(j: &J) -> String {
    match j {
        J::Null => "null".into(),
        J::Bool(b) => b.to_string(),
        J::Num(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{}", *n as i64),
        J::Num(n) => n.to_string(),
        J::Str(s) => format!("\"{}\"", esc(s)),
        J::Arr(xs) => format!("[{}]", xs.iter().map(to_json).collect::<Vec<_>>().join(",")),
        J::Obj(kv) => format!("{{{}}}", kv.iter().map(|(k, v)| format!("\"{}\":{}", esc(k), to_json(v))).collect::<Vec<_>>().join(",")),
    }
}

const TOOLS: &str = r#"[
{"name":"guide","description":"The Brevi language guide. Not needed to start: write plain Python (a .py file) and Brevi builds it. Ask with a word (sections: Strings, Lists, Python, Libraries) for only the lines about it, e.g. when a build error names something; with no word, the whole core.","inputSchema":{"type":"object","properties":{"word":{"type":"string"}}}},
{"name":"check","description":"Compile without running. OK or one JSON error per line: {line, msg, fix}. The server keeps the program: pass code once, then omit it and change it with patch or edit.","inputSchema":{"type":"object","properties":{"code":{"type":"string","description":"Plain Python"},"file":{"type":"string","description":"Default main.py; other files are modules it imports (util.py for import util)"},"fast":{"type":"boolean","description":"Only Brevi's own checks, without rustc"},"fix":{"type":"boolean","description":"Apply every suggested fix to the kept program"},"sandbox":{"type":"boolean","description":"Default true: files and native code need an allow line"}}}},
{"name":"run","description":"Compile and run the kept program (or code, plain Python). With cases, runs each and prints ok or the first differing line.","inputSchema":{"type":"object","properties":{"code":{"type":"string"},"file":{"type":"string"},"stdin":{"type":"string"},"args":{"type":"array","items":{"type":"string"}},"cases":{"type":"array","items":{"type":"object","properties":{"stdin":{"type":"string"},"expect":{"type":"string"}}}},"sandbox":{"type":"boolean","description":"Default true: files and native code need an allow line"}}}},
{"name":"test","description":"Run the test blocks of the kept program (or code).","inputSchema":{"type":"object","properties":{"code":{"type":"string"},"sandbox":{"type":"boolean","description":"Default true: files and native code need an allow line"}}}},
{"name":"patch","description":"Replace each def or class in patch by name in the kept program (or add it); del def name removes one. Then checks: prints what changed and OK or the errors.","inputSchema":{"type":"object","properties":{"patch":{"type":"string"},"file":{"type":"string"},"code":{"type":"string"},"sandbox":{"type":"boolean","description":"Default true: files and native code need an allow line"}},"required":["patch"]}},
{"name":"edit","description":"Replace lines from..to (1-based, inclusive) of the kept program with text (empty text deletes; to = from - 1 inserts before from). Then checks like patch.","inputSchema":{"type":"object","properties":{"from":{"type":"integer"},"to":{"type":"integer"},"text":{"type":"string"},"file":{"type":"string"},"sandbox":{"type":"boolean","description":"Default true: files and native code need an allow line"}},"required":["from","to","text"]}},
{"name":"show","description":"The kept program, or lines from..to of it, with line numbers.","inputSchema":{"type":"object","properties":{"file":{"type":"string"},"from":{"type":"integer"},"to":{"type":"integer"}}}}
]"#;

// The programs the server keeps, by file name, and which one is the program (main.py unless a tool says
// otherwise; main.bv for Brevi code).
#[derive(Default)]
struct Files {
    main: Option<String>,
    all: std::collections::BTreeMap<String, String>,
}

impl Files {
    fn main(&self) -> String {
        self.main.clone().unwrap_or_else(|| "main.py".into())
    }
}

// Brevi rather than Python: an `fn` or `enum` item, a `test "..."` block or a `use "x.bv"` line (none is Python).
fn is_brevi(code: &str) -> bool {
    code.lines().any(|l| {
        let w = l.split(|c: char| c.is_whitespace() || c == '(').next().unwrap_or("");
        (w == "fn" && l.contains('(')) || (w == "enum" && l.trim_end().ends_with(':')) || l.starts_with("test \"") || l.starts_with("use \"")
    })
}

// Runs this same binary on temp copies of the kept files, with a time limit.
fn brevi(files: &Files, cmd: &str, extra: &[String], stdin: &str) -> (String, bool) {
    let (t, err, _) = brevi_file(files, cmd, extra, stdin);
    (t, err)
}

// Also returns the main file as it is after the command (`brevi fix` rewrites it).
fn brevi_file(files: &Files, cmd: &str, extra: &[String], stdin: &str) -> (String, bool, String) {
    let dir = std::env::temp_dir().join(format!("brevi-mcp-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let _ = std::fs::create_dir_all(&dir);
    for (name, text) in &files.all {
        if name.contains("..") || name.starts_with('/') || std::fs::write(dir.join(name), text).is_err() {
            let _ = std::fs::remove_dir_all(&dir);
            return (format!("cannot write {name}"), true, String::new());
        }
    }
    let f = dir.join(files.main());
    let exe = std::env::current_exe().unwrap_or_else(|_| "brevi".into());
    let mut c = Command::new(exe);
    c.arg(cmd).arg(&f).args(extra).current_dir(&dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let Ok(mut child) = c.spawn() else { return ("cannot start brevi".into(), true, String::new()) };
    if let Some(mut si) = child.stdin.take() {
        let s = stdin.to_string();
        std::thread::spawn(move || {
            let _ = si.write_all(s.as_bytes());
        });
    }
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if start.elapsed() > Duration::from_secs(20) => {
                let _ = child.kill();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(_) => break None,
        }
    };
    let out = child.wait_with_output().ok();
    let after = std::fs::read_to_string(&f).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let Some(st) = status else { return ("{\"e\":\"timeout\",\"msg\":\"stopped after 20 s\"}".into(), true, after) };
    let (o, e) = out.map(|o| (String::from_utf8_lossy(&o.stdout).to_string(), String::from_utf8_lossy(&o.stderr).to_string())).unwrap_or_default();
    let mut text = o;
    // Notes like {"line":3,"fixed":"totl -> total"} go to stderr; keep them, they matter to the model.
    if !e.trim().is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&e);
    }
    (text, !st.success(), after)
}

fn call(files: &mut Files, name: &str, a: &J) -> (String, bool) {
    let s = |k: &str| a.get(k).and_then(|v| v.str()).unwrap_or("").to_string();
    let n = |k: &str| a.get(k).and_then(|v| v.num()).map(|x| x as i64);
    // Sandboxed unless the caller says otherwise: no file access or native code without an `allow` line.
    let sandbox: Vec<String> = if a.get("sandbox").map_or(true, |v| v.truthy()) { vec!["--sandbox".into()] } else { vec![] };
    let code = a.get("code").and_then(|v| v.str()).map(String::from).filter(|_| name != "guide");
    let named = Some(s("file")).filter(|f| !f.is_empty());
    // New code replaces the kept file; a call without code uses what is kept. Code with no file name is the
    // program: main.py, or main.bv when it is Brevi. The first file given, or any main.*, is the program too.
    let file = match (&named, &code) {
        (Some(f), _) => f.clone(),
        (None, Some(c)) if files.main.as_deref().map_or(true, |m| m.starts_with("main.")) => {
            if is_brevi(c) { "main.bv".into() } else { "main.py".into() }
        }
        _ => files.main(),
    };
    if let Some(c) = code {
        if files.main.is_none() || file.starts_with("main.") {
            if let Some(old) = files.main.replace(file.clone()).filter(|o| *o != file && o.starts_with("main.")) {
                files.all.remove(&old);
            }
        }
        files.all.insert(file.clone(), c);
    }
    if name != "guide" && !files.main.as_ref().is_some_and(|m| files.all.contains_key(m)) {
        return ("no program yet: pass code once (later calls can leave it out)".into(), true);
    }
    match name {
        "guide" => {
            let w = s("word");
            let g = if w.is_empty() { crate::GUIDE.to_string() } else { crate::guide_lookup(&w) };
            if g.is_empty() { (format!("nothing in the guide for {w}"), true) } else { (g, false) }
        }
        "check" => {
            let mut flags = sandbox.clone();
            if a.get("fast").map_or(false, |v| v.truthy()) {
                flags.push("--fast".into());
            }
            if a.get("fix").map_or(false, |v| v.truthy()) {
                let main = files.main();
                let before = files.all[&main].clone();
                let (t, err, after) = brevi_file(files, "fix", &flags, "");
                let head = if t.trim().is_empty() && !err { "OK".to_string() } else { t.trim_end().to_string() };
                if !after.is_empty() && after != before {
                    files.all.insert(main, after);
                    return (format!("{head}\n(fixes applied to the kept program; show prints it)"), err);
                }
                return (head, err);
            }
            check(files, &flags)
        }
        "run" => {
            let mut extra = sandbox.clone();
            let args: Vec<String> = a.get("args").map(|v| v.arr().iter().filter_map(|x| x.str().map(String::from)).collect()).unwrap_or_default();
            if !args.is_empty() {
                extra.push("--".to_string());
                extra.extend(args);
            }
            match a.get("cases").map(|c| c.arr().to_vec()).filter(|c| !c.is_empty()) {
                Some(cases) => run_cases(files, &extra, &cases),
                None => brevi(files, "run", &extra, &s("stdin")),
            }
        }
        "test" => brevi(files, "test", &sandbox, ""),
        "patch" => {
            let Some(cur) = files.all.get(&file).cloned() else { return (format!("no file {file}"), true) };
            match crate::patch::patch(&cur, &s("patch")) {
                Ok((out, done)) => {
                    files.all.insert(file, out);
                    then_check(files, &sandbox, &done.join(", "))
                }
                Err(m) => (m, true),
            }
        }
        "edit" => {
            let Some(cur) = files.all.get(&file).cloned() else { return (format!("no file {file}"), true) };
            let mut lines: Vec<&str> = cur.split('\n').collect();
            let (from, to) = (n("from").unwrap_or(0), n("to").unwrap_or(-1));
            if from < 1 || to < from - 1 || from as usize > lines.len() + 1 || to as usize > lines.len() {
                return (format!("lines {from}..{to} are outside 1..{}", lines.len()), true);
            }
            let text = s("text");
            let new: Vec<&str> = if text.is_empty() { vec![] } else { text.trim_end_matches('\n').split('\n').collect() };
            let added = new.len();
            lines.splice(from as usize - 1..to as usize, new);
            files.all.insert(file, lines.join("\n"));
            then_check(files, &sandbox, &format!("lines {from}..{to} now {added} line{}", if added == 1 { "" } else { "s" }))
        }
        "show" => {
            let Some(cur) = files.all.get(&file) else { return (format!("no file {file}"), true) };
            let (from, to) = (n("from").unwrap_or(1).max(1) as usize, n("to").map_or(usize::MAX, |t| t.max(0) as usize));
            let text: Vec<String> = cur.split('\n').enumerate().filter(|(i, _)| i + 1 >= from && i + 1 <= to).map(|(i, l)| format!("{} {l}", i + 1)).collect();
            (text.join("\n"), false)
        }
        _ => (format!("unknown tool {name}"), true),
    }
}

fn check(files: &Files, flags: &[String]) -> (String, bool) {
    let (t, err) = brevi(files, "check", flags, "");
    (if t.trim().is_empty() && !err { "OK".into() } else { t }, err)
}

// After a change: what changed, then OK or the errors, in one reply.
fn then_check(files: &Files, flags: &[String], what: &str) -> (String, bool) {
    let (t, err) = check(files, flags);
    (format!("{what}\n{}", t.trim_end()), err)
}

// Builds once, then runs each case. Prints ok per case, or the first line that differs.
fn run_cases(files: &Files, extra: &[String], cases: &[J]) -> (String, bool) {
    let mut flags: Vec<String> = extra.iter().take_while(|a| *a != "--").cloned().collect();
    let args: Vec<String> = extra.iter().skip_while(|a| *a != "--").cloned().collect();
    let bin = std::env::temp_dir().join(format!("brevi-mcp-bin-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    flags.extend(["-o".to_string(), bin.to_string_lossy().to_string()]);
    let (t, err) = brevi(files, "build", &flags, "");
    if err {
        return (t, true);
    }
    let mut out = vec![];
    let mut failed = 0;
    for (i, c) in cases.iter().enumerate() {
        let stdin = c.get("stdin").and_then(|v| v.str()).unwrap_or("");
        let want = c.get("expect").and_then(|v| v.str()).unwrap_or("");
        let (got, code) = run_bin(&bin, &args[args.len().min(1)..], stdin);
        let (gl, wl): (Vec<&str>, Vec<&str>) = (got.trim_end().lines().map(str::trim_end).collect(), want.trim_end().lines().map(str::trim_end).collect());
        if gl == wl {
            out.push(format!("case {}: ok", i + 1));
            continue;
        }
        failed += 1;
        let k = gl.iter().zip(&wl).take_while(|(g, w)| g == w).count();
        let g = gl.get(k).map_or("(no more output)".to_string(), |l| format!("{l:?}"));
        let w = wl.get(k).map_or("(no more output)".to_string(), |l| format!("{l:?}"));
        let crash = if code { " (program failed)" } else { "" };
        out.push(format!("case {}: line {} want {w} got {g}{crash}", i + 1, k + 1));
    }
    let _ = std::fs::remove_file(&bin);
    out.push(format!("passed {}/{}", cases.len() - failed, cases.len()));
    (out.join("\n"), failed > 0)
}

// Runs a built program with a time limit; stdout plus any error line, and whether it failed.
fn run_bin(bin: &std::path::Path, args: &[String], stdin: &str) -> (String, bool) {
    let Ok(mut child) = Command::new(bin).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() else {
        return ("cannot start the program".into(), true);
    };
    if let Some(mut si) = child.stdin.take() {
        let s = stdin.to_string();
        std::thread::spawn(move || {
            let _ = si.write_all(s.as_bytes());
        });
    }
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() > Duration::from_secs(10) => {
                let _ = child.kill();
                return ("{\"e\":\"timeout\",\"msg\":\"stopped after 10 s\"}".into(), true);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(2)),
            Err(_) => break,
        }
    }
    let Ok(o) = child.wait_with_output() else { return (String::new(), true) };
    let mut t = String::from_utf8_lossy(&o.stdout).to_string();
    let e = String::from_utf8_lossy(&o.stderr);
    if !o.status.success() && !e.trim().is_empty() {
        t.push_str(e.trim_end());
    }
    (t, !o.status.success())
}

pub fn serve() {
    let mut files = Files::default();
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Some(msg) = parse(&line) else {
            let _ = writeln!(out, r#"{{"jsonrpc":"2.0","id":null,"error":{{"code":-32700,"message":"parse error"}}}}"#);
            continue;
        };
        let Some(id) = msg.get("id") else { continue }; // a notification: no reply
        let id = to_json(id);
        let method = msg.get("method").and_then(|m| m.str()).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(J::Obj(vec![]));
        let result = match method {
            "initialize" => {
                let v = params.get("protocolVersion").and_then(|v| v.str()).unwrap_or("2025-06-18");
                Ok(format!(r#"{{"protocolVersion":"{}","capabilities":{{"tools":{{}}}},"serverInfo":{{"name":"brevi","version":"{}"}},"instructions":"Write plain Python: Brevi builds it to a native program, no guide needed. Write the plain solution; do not hand-optimise it for speed, Brevi makes it fast. Pass your program as code once; then fix errors with patch (whole defs by name) or edit (lines), which change only what you send and check again."}}"#, esc(v), env!("CARGO_PKG_VERSION")))
            }
            "ping" => Ok("{}".into()),
            "tools/list" => Ok(format!(r#"{{"tools":{}}}"#, TOOLS.replace('\n', ""))),
            "tools/call" => {
                let name = params.get("name").and_then(|v| v.str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(J::Obj(vec![]));
                let (text, err) = call(&mut files, name, &args);
                Ok(format!(r#"{{"content":[{{"type":"text","text":"{}"}}],"isError":{err}}}"#, esc(&text)))
            }
            _ => Err(format!(r#"{{"code":-32601,"message":"no method {}"}}"#, esc(method))),
        };
        let reply = match result {
            Ok(r) => format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{r}}}"#),
            Err(e) => format!(r#"{{"jsonrpc":"2.0","id":{id},"error":{e}}}"#),
        };
        let _ = writeln!(out, "{reply}");
        let _ = out.flush();
    }
}
