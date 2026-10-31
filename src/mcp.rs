// `brevi mcp`: a Model Context Protocol server on stdin/stdout (JSON-RPC, one message per line).
// Tools: guide, check, run, test, patch. Each takes the program as text, so a client needs no files.
use crate::diag::esc;
use crate::json::{parse, J};
use std::io::{BufRead, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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
{"name":"guide","description":"The Brevi language guide. Read it once (no word) before writing Brevi; with a word, only the guide lines about it.","inputSchema":{"type":"object","properties":{"word":{"type":"string","description":"A name or section, like re_list or Tests"}}}},
{"name":"check","description":"Compile a Brevi program without running it. Prints OK or one JSON error per line: {line, msg, fix}.","inputSchema":{"type":"object","properties":{"code":{"type":"string"},"fast":{"type":"boolean","description":"Only Brevi's own checks, without rustc"}},"required":["code"]}},
{"name":"run","description":"Compile and run a Brevi program with the given stdin. Prints its output, or the JSON errors.","inputSchema":{"type":"object","properties":{"code":{"type":"string"},"stdin":{"type":"string"},"args":{"type":"array","items":{"type":"string"}}},"required":["code"]}},
{"name":"test","description":"Run the test blocks in a Brevi program.","inputSchema":{"type":"object","properties":{"code":{"type":"string"}},"required":["code"]}},
{"name":"patch","description":"Replace each fn, type, enum or test in `patch` by name in `code` (or add it); `del fn name` removes one. Returns the new program.","inputSchema":{"type":"object","properties":{"code":{"type":"string"},"patch":{"type":"string"}},"required":["code","patch"]}}
]"#;

// Runs this same binary on a temp copy of the program, with a time limit.
fn brevi(cmd: &str, code: &str, extra: &[String], stdin: &str) -> (String, bool) {
    let dir = std::env::temp_dir().join(format!("brevi-mcp-{}-{}", std::process::id(), Instant::now().elapsed().as_nanos()));
    let _ = std::fs::create_dir_all(&dir);
    let f = dir.join("main.bv");
    if std::fs::write(&f, code).is_err() {
        return ("cannot write a temp file".into(), true);
    }
    let exe = std::env::current_exe().unwrap_or_else(|_| "brevi".into());
    let mut c = Command::new(exe);
    c.arg(cmd).arg(&f).args(extra).current_dir(&dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let Ok(mut child) = c.spawn() else { return ("cannot start brevi".into(), true) };
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
    let _ = std::fs::remove_dir_all(&dir);
    let Some(st) = status else { return ("{\"e\":\"timeout\",\"msg\":\"stopped after 20 s\"}".into(), true) };
    let (o, e) = out.map(|o| (String::from_utf8_lossy(&o.stdout).to_string(), String::from_utf8_lossy(&o.stderr).to_string())).unwrap_or_default();
    let mut text = o;
    // Notes like {"line":3,"fixed":"totl -> total"} go to stderr; keep them, they matter to the model.
    if !e.trim().is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&e);
    }
    (text, !st.success())
}

fn call(name: &str, a: &J) -> (String, bool) {
    let s = |k: &str| a.get(k).and_then(|v| v.str()).unwrap_or("").to_string();
    match name {
        "guide" => {
            let w = s("word");
            let g = if w.is_empty() { crate::GUIDE.to_string() } else { crate::guide_lookup(&w) };
            if g.is_empty() { (format!("nothing in the guide for {w}"), true) } else { (g, false) }
        }
        "check" => {
            let fast = a.get("fast").map_or(false, |v| v.truthy());
            let flags: Vec<String> = if fast { vec!["--fast".into()] } else { vec![] };
            let (t, err) = brevi("check", &s("code"), &flags, "");
            (if t.trim().is_empty() && !err { "OK".into() } else { t }, err)
        }
        "run" => {
            let mut extra = vec![];
            let args: Vec<String> = a.get("args").map(|v| v.arr().iter().filter_map(|x| x.str().map(String::from)).collect()).unwrap_or_default();
            if !args.is_empty() {
                extra.push("--".to_string());
                extra.extend(args);
            }
            brevi("run", &s("code"), &extra, &s("stdin"))
        }
        "test" => brevi("test", &s("code"), &[], ""),
        "patch" => match crate::patch::patch(&s("code"), &s("patch")) {
            Ok((out, done)) => (format!("{}\n---\n{out}", done.join(", ")), false),
            Err(m) => (m, true),
        },
        _ => (format!("unknown tool {name}"), true),
    }
}

pub fn serve() {
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
                Ok(format!(r#"{{"protocolVersion":"{}","capabilities":{{"tools":{{}}}},"serverInfo":{{"name":"brevi","version":"{}"}},"instructions":"Call guide once before writing Brevi, then check or run your program and fix only the lines each error names."}}"#, esc(v), env!("CARGO_PKG_VERSION")))
            }
            "ping" => Ok("{}".into()),
            "tools/list" => Ok(format!(r#"{{"tools":{}}}"#, TOOLS.replace('\n', ""))),
            "tools/call" => {
                let name = params.get("name").and_then(|v| v.str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(J::Obj(vec![]));
                let (text, err) = call(name, &args);
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
