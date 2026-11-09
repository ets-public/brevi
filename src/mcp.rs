// `brevi mcp`: a Model Context Protocol server on stdin/stdout (JSON-RPC 2.0, one message per line).
// Tools: guide, check, run, test, patch, edit, show. The server keeps the program: send `code` once, then
// change it with patch (whole defs by name) or edit (a range of lines), so a retry costs only the change.
// Code is plain Python (kept as main.py, built like `brevi run main.py`) unless it is Brevi (`fn` items, or a
// file name ending .bv).
//
// Robustness rules (tests/mcp_test.py checks each):
// - stdout carries only JSON-RPC replies, one per line; children never write to it (their output is piped).
// - Bad input never stops the server: invalid UTF-8 or JSON gets a parse error, anything that is not a
//   request object gets an invalid-request error, and a panic in a tool becomes an error result.
// - Tool calls run one at a time, in order, on a worker thread; ping, initialize and tools/list are answered
//   at once even while a call runs, and notifications/cancelled stops the call it names.
// - Every child runs in its own process group with a time limit; on timeout, cancel or exit the whole tree
//   (rustc, the program, python3) is killed. Output is read while the program runs (so a program printing a
//   lot cannot block) and long output is cut in the middle.
// - Files are written to one work directory per server, which is removed, with its build directories, when
//   the server stops. Directories left by servers that died are removed at start.
use crate::diag::esc;
use crate::json::{parse, J};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

// The protocol versions this server speaks through the initialize handshake; the newest is offered when a
// client asks for another.
const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
// The versions without a handshake (2026-07-28 on): each request carries its version, the client's info and
// capabilities in params._meta, `server/discover` replaces initialize, and every result says
// "resultType":"complete" and names the server in its _meta. Errors and shapes follow the mcp 2.3.0 SDK.
const MODERN: [&str; 1] = ["2026-07-28"];
const K_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
const K_CAPS: &str = "io.modelcontextprotocol/clientCapabilities";
const INSTRUCTIONS: &str = "Write plain Python: Brevi compiles it to native code, which runs plain loops several times faster than CPython (1.7x to 39.5x, median 8.5x, on our 12 compute benchmarks), so write the plain, direct solution and do not hand-optimise it. No guide needed (it covers Brevi syntax, .bv files). Pass your program as code once; then fix errors with patch (whole defs by name) or edit (lines), which change only what you send and check again.";

// A 2026-07-28 request's envelope: Ok(true) for one, Ok(false) for a request without one (the handshake era),
// Err(the error object) when the envelope names a version this server lacks or leaves a key out.
fn envelope(params: &J) -> Result<bool, String> {
    let Some(meta) = params.get("_meta") else { return Ok(false) };
    let Some(v) = meta.get(K_VERSION) else { return Ok(false) };
    let asked = v.str().unwrap_or("");
    if !MODERN.contains(&asked) {
        let req = if v.str().is_some() { format!("\"{}\"", esc(asked)) } else { to_json(v) };
        return Err(format!(r#"{{"code":-32022,"message":"Unsupported protocol version","data":{{"supported":["{}"],"requested":{req}}}}}"#, MODERN.join("\",\"")));
    }
    if !matches!(meta.get(K_CAPS), Some(J::Obj(_))) {
        return Err(format!(r#"{{"code":-32602,"message":"params._meta is missing the required envelope key(s): {K_CAPS}"}}"#));
    }
    Ok(true)
}

// A result as the 2026-07-28 protocol sends it: resultType and the server's name added (cache fields for
// lists and discovery, which say how long a client may keep them: not at all).
fn stamp(result: &str, cached: bool) -> String {
    let body = result.trim_end().strip_suffix('}').unwrap_or(result);
    let sep = if body.trim_end().ends_with('{') { "" } else { "," };
    let cache = if cached { r#""cacheScope":"private","ttlMs":0,"# } else { "" };
    format!(r#"{body}{sep}{cache}"resultType":"complete","_meta":{{"io.modelcontextprotocol/serverInfo":{{"name":"brevi","title":"Brevi","version":"{}"}}}}}}"#, env!("CARGO_PKG_VERSION"))
}

// What a reply shows of long output: the start and the end.
const CLIP_HEAD: usize = 16_000;
const CLIP_TAIL: usize = 4_000;
// What is kept of a program's output for comparing with a case's expected output.
const KEEP: usize = 8 << 20;

pub(crate) fn to_json(j: &J) -> String {
    match j {
        J::Null => "null".into(),
        J::Bool(b) => b.to_string(),
        J::Num(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => format!("{}", *n as i64),
        J::Num(n) => n.to_string(),
        J::Str(s) => format!("\"{}\"", esc(s)),
        J::Arr(xs) => format!("[{}]", xs.iter().map(to_json).collect::<Vec<_>>().join(",")),
        J::Obj(kv) => format!("{{{}}}", kv.iter().map(|(k, v)| format!("\"{}\":{}", esc(k), to_json(v))).collect::<Vec<_>>().join(",")),
    }
}

const TOOLS: &str = r#"[
{"name":"guide","title":"Brevi guide","description":"The Brevi language guide. Not needed to start: write plain Python (a .py file) and Brevi builds it to native code, which runs plain loops several times faster than CPython (1.7x to 39.5x, median 8.5x, on our 12 compute benchmarks), so write the plain, direct solution and do not hand-optimise it. Ask with a word (sections: Strings, Lists, Python, Libraries) for only the lines about it, e.g. when a build error names something; with no word, the whole core.","inputSchema":{"type":"object","properties":{"word":{"type":"string"}},"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":false}},
{"name":"check","title":"Check program","description":"Compile without running. OK or one JSON error per line: {line, msg, fix}. The server keeps the program: pass code once, then omit it and change it with patch or edit.","inputSchema":{"type":"object","properties":{"code":{"type":"string","description":"Plain Python"},"file":{"type":"string","description":"Default main.py; other files are modules it imports (util.py for import util)"},"fast":{"type":"boolean","description":"Only Brevi's own checks, without rustc"},"fix":{"type":"boolean","description":"Apply every suggested fix to the kept program"},"sandbox":{"type":"boolean","description":"Default true: files, native code and Python libraries need allow or allow_py"},"allow":{"type":"array","items":{"type":"string","enum":["read","write","net","native"]},"description":"Capabilities granted under the sandbox; an allow line in the code grants nothing"},"allow_py":{"type":"array","items":{"type":"string"},"description":"Python libraries CPython may run under the sandbox (requests, pydantic)"},"allow_net":{"type":"array","items":{"type":"string"},"description":"Hosts the program may reach under the sandbox (api.example.com); allow net grants every host"},"allow_read":{"type":"array","items":{"type":"string"},"description":"Folders or files the program may read under the sandbox (relative to where brevi mcp started); allow read grants every file"},"allow_write":{"type":"array","items":{"type":"string"},"description":"Folders or files the program may write under the sandbox; allow write grants every file"}},"additionalProperties":false},"annotations":{"openWorldHint":false}},
{"name":"run","title":"Run program","description":"Compile and run the kept program (or code, plain Python). With cases, runs each and prints ok or the first differing line.","inputSchema":{"type":"object","properties":{"code":{"type":"string"},"file":{"type":"string"},"stdin":{"type":"string"},"args":{"type":"array","items":{"type":"string"}},"cases":{"type":"array","items":{"type":"object","properties":{"stdin":{"type":"string"},"expect":{"type":"string"}},"additionalProperties":false}},"sandbox":{"type":"boolean","description":"Default true: files, native code and Python libraries need allow or allow_py"},"allow":{"type":"array","items":{"type":"string","enum":["read","write","net","native"]},"description":"Capabilities granted under the sandbox; an allow line in the code grants nothing"},"allow_py":{"type":"array","items":{"type":"string"},"description":"Python libraries CPython may run under the sandbox (requests, pydantic)"},"allow_net":{"type":"array","items":{"type":"string"},"description":"Hosts the program may reach under the sandbox (api.example.com); allow net grants every host"},"allow_read":{"type":"array","items":{"type":"string"},"description":"Folders or files the program may read under the sandbox (relative to where brevi mcp started); allow read grants every file"},"allow_write":{"type":"array","items":{"type":"string"},"description":"Folders or files the program may write under the sandbox; allow write grants every file"},"limits":{"type":"object","properties":{"cpu":{"type":"number","description":"CPU seconds"},"mem":{"type":"number","description":"Memory, MB (Linux)"},"time":{"type":"number","description":"Wall-clock seconds for the program"},"output":{"type":"number","description":"KB of stdout and stderr together"},"file":{"type":"number","description":"Largest file it may write, MB"},"written":{"type":"number","description":"MB written other than stdout and stderr (Linux)"},"net":{"type":"number","description":"MB sent and received over the network"},"threads":{"type":"number","description":"Threads at once (Linux)"},"fds":{"type":"number","description":"Open files at once"},"files":{"type":"number","description":"Files it may create"}},"additionalProperties":false,"description":"Resource limits on the program run (not the build); the program is stopped at the first one it reaches"},"timeout":{"type":"integer","minimum":1,"maximum":600,"description":"Seconds for build and run (default 20; per case 10)"}},"additionalProperties":false}},
{"name":"test","title":"Run tests","description":"Run the test blocks of the kept program (or code).","inputSchema":{"type":"object","properties":{"code":{"type":"string"},"file":{"type":"string"},"sandbox":{"type":"boolean","description":"Default true: files, native code and Python libraries need allow or allow_py"},"allow":{"type":"array","items":{"type":"string","enum":["read","write","net","native"]},"description":"Capabilities granted under the sandbox; an allow line in the code grants nothing"},"allow_py":{"type":"array","items":{"type":"string"},"description":"Python libraries CPython may run under the sandbox (requests, pydantic)"},"allow_net":{"type":"array","items":{"type":"string"},"description":"Hosts the program may reach under the sandbox (api.example.com); allow net grants every host"},"allow_read":{"type":"array","items":{"type":"string"},"description":"Folders or files the program may read under the sandbox (relative to where brevi mcp started); allow read grants every file"},"allow_write":{"type":"array","items":{"type":"string"},"description":"Folders or files the program may write under the sandbox; allow write grants every file"},"limits":{"type":"object","properties":{"cpu":{"type":"number","description":"CPU seconds"},"mem":{"type":"number","description":"Memory, MB (Linux)"},"time":{"type":"number","description":"Wall-clock seconds for the program"},"output":{"type":"number","description":"KB of stdout and stderr together"},"file":{"type":"number","description":"Largest file it may write, MB"},"written":{"type":"number","description":"MB written other than stdout and stderr (Linux)"},"net":{"type":"number","description":"MB sent and received over the network"},"threads":{"type":"number","description":"Threads at once (Linux)"},"fds":{"type":"number","description":"Open files at once"},"files":{"type":"number","description":"Files it may create"}},"additionalProperties":false,"description":"Resource limits on the program run (not the build); the program is stopped at the first one it reaches"},"timeout":{"type":"integer","minimum":1,"maximum":600,"description":"Seconds (default 20)"}},"additionalProperties":false}},
{"name":"patch","title":"Patch program","description":"Replace each def or class in patch by name in the kept program (or add it); del def name removes one. Then checks: prints what changed and OK or the errors.","inputSchema":{"type":"object","properties":{"patch":{"type":"string"},"file":{"type":"string"},"code":{"type":"string"},"sandbox":{"type":"boolean","description":"Default true: files, native code and Python libraries need allow or allow_py"},"allow":{"type":"array","items":{"type":"string","enum":["read","write","net","native"]},"description":"Capabilities granted under the sandbox; an allow line in the code grants nothing"},"allow_py":{"type":"array","items":{"type":"string"},"description":"Python libraries CPython may run under the sandbox (requests, pydantic)"},"allow_net":{"type":"array","items":{"type":"string"},"description":"Hosts the program may reach under the sandbox (api.example.com); allow net grants every host"},"allow_read":{"type":"array","items":{"type":"string"},"description":"Folders or files the program may read under the sandbox (relative to where brevi mcp started); allow read grants every file"},"allow_write":{"type":"array","items":{"type":"string"},"description":"Folders or files the program may write under the sandbox; allow write grants every file"}},"required":["patch"],"additionalProperties":false},"annotations":{"openWorldHint":false}},
{"name":"edit","title":"Edit lines","description":"Replace lines from..to (1-based, inclusive) of the kept program with text (empty text deletes; to = from - 1 inserts before from). Then checks like patch.","inputSchema":{"type":"object","properties":{"from":{"type":"integer","minimum":1},"to":{"type":"integer","minimum":0},"text":{"type":"string"},"file":{"type":"string"},"sandbox":{"type":"boolean","description":"Default true: files, native code and Python libraries need allow or allow_py"},"allow":{"type":"array","items":{"type":"string","enum":["read","write","net","native"]},"description":"Capabilities granted under the sandbox; an allow line in the code grants nothing"},"allow_py":{"type":"array","items":{"type":"string"},"description":"Python libraries CPython may run under the sandbox (requests, pydantic)"},"allow_net":{"type":"array","items":{"type":"string"},"description":"Hosts the program may reach under the sandbox (api.example.com); allow net grants every host"},"allow_read":{"type":"array","items":{"type":"string"},"description":"Folders or files the program may read under the sandbox (relative to where brevi mcp started); allow read grants every file"},"allow_write":{"type":"array","items":{"type":"string"},"description":"Folders or files the program may write under the sandbox; allow write grants every file"}},"required":["from","to","text"],"additionalProperties":false},"annotations":{"openWorldHint":false}},
{"name":"verify","title":"Verify against CPython","description":"Proof the native build answers as CPython does: builds the kept program (or code, plain Python) natively, runs it and python3 on each input, and compares stdout, exit code and stderr (CPython runs twice, so output that changes run to run is reported, not certified). One JSON line per input, then a certificate {verify: identical, differs, unstable, timeout, limit or not built; sha256, path, speedup}. Runs CPython on the code, which the sandbox cannot hold, so it needs sandbox false.","inputSchema":{"type":"object","properties":{"code":{"type":"string","description":"Plain Python"},"file":{"type":"string"},"inputs":{"type":"array","items":{"type":"string"},"description":"stdin for each run (default one run, empty stdin)"},"args":{"type":"array","items":{"type":"string"}},"sandbox":{"type":"boolean","description":"Must be false: verify runs python3 on the code"},"once":{"type":"boolean","description":"Run CPython once, not twice"},"limits":{"type":"object","properties":{"cpu":{"type":"number","description":"CPU seconds"},"mem":{"type":"number","description":"Memory, MB (Linux)"},"time":{"type":"number","description":"Wall-clock seconds for the program"},"output":{"type":"number","description":"KB of stdout and stderr together"},"file":{"type":"number","description":"Largest file it may write, MB"},"written":{"type":"number","description":"MB written other than stdout and stderr (Linux)"},"net":{"type":"number","description":"MB sent and received over the network"},"threads":{"type":"number","description":"Threads at once (Linux)"},"fds":{"type":"number","description":"Open files at once"},"files":{"type":"number","description":"Files it may create"}},"additionalProperties":false,"description":"Resource limits on the program run (not the build); the program is stopped at the first one it reaches"},"timeout":{"type":"integer","minimum":1,"maximum":600,"description":"Seconds for each run (default 20)"}},"additionalProperties":false},"annotations":{"openWorldHint":false}},
{"name":"show","title":"Show program","description":"The kept program, or lines from..to of it, with line numbers.","inputSchema":{"type":"object","properties":{"file":{"type":"string"},"from":{"type":"integer","minimum":1},"to":{"type":"integer","minimum":0}},"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":false}}
]"#;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Str,
    Int,
    Bool,
    Strs,
    Cases,
    Limits,
}

// Each tool's arguments: name, kind, required.
fn spec(tool: &str) -> Option<&'static [(&'static str, Kind, bool)]> {
    use Kind::*;
    const SB: (&str, Kind, bool) = ("sandbox", Bool, false);
    const AL: (&str, Kind, bool) = ("allow", Strs, false);
    const AP: (&str, Kind, bool) = ("allow_py", Strs, false);
    const AN: (&str, Kind, bool) = ("allow_net", Strs, false);
    const AR: (&str, Kind, bool) = ("allow_read", Strs, false);
    const AW: (&str, Kind, bool) = ("allow_write", Strs, false);
    const LI: (&str, Kind, bool) = ("limits", Limits, false);
    Some(match tool {
        "guide" => &[("word", Str, false)],
        "check" => &[("code", Str, false), ("file", Str, false), ("fast", Bool, false), ("fix", Bool, false), SB, AL, AP, AN, AR, AW],
        "run" => &[("code", Str, false), ("file", Str, false), ("stdin", Str, false), ("args", Strs, false), ("cases", Cases, false), SB, AL, AP, AN, AR, AW, LI, ("timeout", Int, false)],
        "test" => &[("code", Str, false), ("file", Str, false), SB, AL, AP, AN, AR, AW, LI, ("timeout", Int, false)],
        "patch" => &[("patch", Str, true), ("file", Str, false), ("code", Str, false), SB, AL, AP, AN, AR, AW],
        "edit" => &[("from", Int, true), ("to", Int, true), ("text", Str, true), ("file", Str, false), SB, AL, AP, AN, AR, AW],
        "show" => &[("file", Str, false), ("from", Int, false), ("to", Int, false)],
        "verify" => &[("code", Str, false), ("file", Str, false), ("inputs", Strs, false), ("args", Strs, false), SB, ("once", Bool, false), LI, ("timeout", Int, false)],
        _ => return None,
    })
}

// Checks a call's arguments against the tool's spec, so a misspelt or mistyped argument is reported instead
// of silently ignored. Forgiving where the meaning is clear: null is the same as leaving an argument out,
// numbers are accepted as text, and "3" / "true" as an integer / boolean.
fn validate(tool: &str, a: &J) -> Result<J, String> {
    let sp = spec(tool).unwrap_or(&[]);
    let kv = match a {
        J::Obj(kv) => kv,
        J::Null => return Ok(J::Obj(vec![])),
        _ => return Err("arguments must be an object".into()),
    };
    let names = || sp.iter().map(|s| s.0).collect::<Vec<_>>().join(", ");
    let text = |v: &J| match v {
        J::Str(s) => Some(s.clone()),
        J::Num(_) | J::Bool(_) => Some(to_json(v)),
        _ => None,
    };
    let mut out = vec![];
    for (k, v) in kv {
        let Some(&(_, kind, _)) = sp.iter().find(|s| s.0 == k) else {
            return Err(format!("{tool} has no argument {k:?}; it takes: {}", names()));
        };
        if matches!(v, J::Null) {
            continue;
        }
        let bad = |want: &str| format!("{k} must be {want}");
        let v = match kind {
            Kind::Str => J::Str(text(v).ok_or_else(|| bad("a string"))?),
            Kind::Int => match v {
                J::Num(n) if n.fract() == 0.0 => J::Num(*n),
                J::Str(s) if s.trim().parse::<i64>().is_ok() => J::Num(s.trim().parse::<i64>().unwrap() as f64),
                _ => return Err(bad("a whole number")),
            },
            Kind::Bool => match v {
                J::Bool(b) => J::Bool(*b),
                J::Str(s) if s == "true" || s == "false" => J::Bool(s == "true"),
                _ => return Err(bad("true or false")),
            },
            Kind::Strs => match v {
                J::Arr(xs) => J::Arr(xs.iter().map(|x| text(x).map(J::Str)).collect::<Option<_>>().ok_or_else(|| bad("a list of strings"))?),
                _ => return Err(bad("a list of strings")),
            },
            Kind::Cases => {
                let J::Arr(xs) = v else { return Err(bad("a list of {stdin, expect} objects")) };
                let mut cases = vec![];
                for (i, c) in xs.iter().enumerate() {
                    let J::Obj(ckv) = c else { return Err(format!("case {} must be an object {{stdin, expect}}", i + 1)) };
                    let mut o = vec![];
                    for (ck, cv) in ckv {
                        if ck != "stdin" && ck != "expect" {
                            return Err(format!("case {} has {ck:?}; a case takes stdin and expect", i + 1));
                        }
                        if !matches!(cv, J::Null) {
                            o.push((ck.clone(), J::Str(text(cv).ok_or_else(|| format!("case {} {ck} must be a string", i + 1))?)));
                        }
                    }
                    cases.push(J::Obj(o));
                }
                J::Arr(cases)
            }
            // {cpu, mem, time, output, file}: numbers (seconds, MB, seconds, KB, MB) or text such as "512kb".
            Kind::Limits => {
                let J::Obj(lkv) = v else { return Err(bad("an object such as {\"cpu\": 2, \"mem\": 256, \"time\": 5, \"output\": 64, \"file\": 10}")) };
                let mut o = vec![];
                for (lk, lv) in lkv {
                    if !["cpu", "mem", "time", "output", "file", "written", "net", "threads", "fds", "files"].contains(&lk.as_str()) {
                        return Err(format!("limits has {lk:?}; it takes cpu (seconds), mem (MB), time (seconds), output (KB), file (MB), written (MB), net (MB), threads, fds and files"));
                    }
                    if matches!(lv, J::Null) {
                        continue;
                    }
                    let t = text(lv).filter(|t| !t.contains([',', '=', ' '])).ok_or_else(|| format!("limits {lk} must be a number"))?;
                    let mut l = crate::proc::Limits::default();
                    l.parse(&format!("{lk}={t}")).map_err(|m| m.replace("--limit takes cpu=SECS, mem=MB, time=SECS, output=KB, file=MB, written=MB, net=MB, threads=N, fds=N, files=N, comma separated (cpu=2,mem=256,time=5)", &format!("limits {lk} must be a number above 0")).replace("--limit", "limits"))?;
                    o.push((lk.clone(), J::Str(t)));
                }
                J::Obj(o)
            }
        };
        out.push((k.clone(), v));
    }
    for (k, _, req) in sp {
        if *req && !out.iter().any(|(o, _)| o == k) {
            return Err(format!("{tool} needs {k}"));
        }
    }
    Ok(J::Obj(out))
}

// A file name the server will write: relative, inside the work directory, nothing odd.
fn file_ok(name: &str) -> Result<(), String> {
    let bad = name.is_empty()
        || name.len() > 200
        || name.contains(['\\', ':', '\0'])
        || name.starts_with('/')
        || name.split('/').any(|p| p.is_empty() || p == "." || p == ".." || p.starts_with('.'))
        || name.chars().any(char::is_control);
    if bad {
        return Err(format!("file {name:?} is not allowed: use a plain relative name like util.py or pkg/util.py"));
    }
    Ok(())
}

// The programs the server keeps, by file name, and which one is the program (main.py unless a tool says
// otherwise; main.bv for Brevi code).
#[derive(Default)]
struct Files {
    main: Option<String>,
    all: BTreeMap<String, String>,
    // Every main file name built, so their build directories can be removed at the end.
    built: BTreeSet<String>,
    // The path report of the call's last build ({"path":"typed"}, ...; see say_path in main.rs), sent as the
    // result's structuredContent.
    path: Option<String>,
    // A plain run's own outcome (stdout, stderr, exit_code, ms, as JSON fields), also in structuredContent,
    // so a client such as the Python SDK gets them apart instead of the merged text.
    run: Option<String>,
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

struct Cfg {
    timeout: u64,
    mem_mb: u64,
}
static CFG: OnceLock<Cfg> = OnceLock::new();
fn cfg() -> &'static Cfg {
    CFG.get().expect("config")
}

// The call being run: its child's process group, and whether it was cancelled.
static CHILD: AtomicU32 = AtomicU32::new(0);
static CANCELLED: AtomicBool = AtomicBool::new(false);
// Set when the client has gone: queued calls are dropped.
static SHUTTING: AtomicBool = AtomicBool::new(false);

// Building the runtime libraries the first time (after an install or upgrade) takes rustc 5 to 25 s, more
// than a call's time limit, and a call stopped mid-way would leave them unbuilt for the next call too. So the
// server builds them once at start, in the background, and calls that build wait for that first (the wait
// does not count against their limit).
static WARM: (Mutex<bool>, std::sync::Condvar) = (Mutex::new(false), std::sync::Condvar::new());

fn warm_dir() -> PathBuf {
    std::env::temp_dir().join(format!("brevi-mcp-{}-warm", std::process::id()))
}

fn warm_up() {
    let dir = warm_dir();
    let _ = std::fs::create_dir_all(&dir);
    let f = dir.join("warm.py");
    let _ = std::fs::write(&f, "print(sum(range(3)))\n");
    let exe = std::env::current_exe().unwrap_or_else(|_| "brevi".into());
    // The typed build's runtime library, then the dynamic one.
    for dynamic in [false, true] {
        let mut c = Command::new(&exe);
        c.arg("build").arg(&f).arg("-o").arg(dir.join("warm")).current_dir(&dir).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if dynamic {
            c.env("BREVI_DYN", "1");
        }
        crate::proc::own_group(&mut c);
        crate::proc::die_with_parent(&mut c);
        if let Ok(mut ch) = c.spawn() {
            let start = Instant::now();
            loop {
                match ch.try_wait() {
                    Ok(None) if start.elapsed() < Duration::from_secs(300) && !SHUTTING.load(Ordering::SeqCst) => std::thread::sleep(Duration::from_millis(20)),
                    Ok(None) => {
                        crate::proc::stop(&mut ch);
                        let _ = ch.wait();
                        break;
                    }
                    _ => break,
                }
            }
        }
    }
    *WARM.0.lock().unwrap_or_else(|e| e.into_inner()) = true;
    WARM.1.notify_all();
}

// Waits for warm_up (at most 5 minutes; a cancel ends the wait).
fn wait_warm() {
    let mut done = WARM.0.lock().unwrap_or_else(|e| e.into_inner());
    let start = Instant::now();
    while !*done && !CANCELLED.load(Ordering::SeqCst) && start.elapsed() < Duration::from_secs(300) {
        done = WARM.1.wait_timeout(done, Duration::from_millis(50)).unwrap_or_else(|e| e.into_inner()).0;
    }
}

fn work_dir() -> &'static PathBuf {
    static W: OnceLock<PathBuf> = OnceLock::new();
    W.get_or_init(|| std::env::temp_dir().join(format!("brevi-mcp-{}", std::process::id())))
}

// Writes the kept files to a clean work directory.
fn write_files(files: &Files) -> Result<PathBuf, String> {
    let dir = work_dir().clone();
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot make the work directory {}: {e}", dir.display()))?;
    for (name, text) in &files.all {
        file_ok(name)?;
        let p = dir.join(name);
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&p, text).map_err(|e| format!("cannot write {name}: {e}"))?;
    }
    Ok(dir)
}

// The output of one stream: everything up to KEEP bytes, then only the last CLIP_TAIL bytes and a count.
#[derive(Default)]
struct Cap {
    head: Vec<u8>,
    tail: std::collections::VecDeque<u8>,
    total: usize,
}

impl Cap {
    fn text(&self) -> String {
        let mut s = String::from_utf8_lossy(&self.head).into_owned();
        if self.total > self.head.len() {
            let tail: Vec<u8> = self.tail.iter().copied().collect();
            s.push_str(&format!("\n[... {} bytes cut ...]\n", self.total - self.head.len() - tail.len()));
            s.push_str(&String::from_utf8_lossy(&tail));
        }
        s
    }
}

fn reader(mut r: impl Read + Send + 'static, seen: Arc<std::sync::atomic::AtomicU64>) -> mpsc::Receiver<Cap> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut cap = Cap::default();
        let mut buf = [0u8; 16384];
        while let Ok(n) = r.read(&mut buf) {
            if n == 0 {
                break;
            }
            cap.total += n;
            seen.fetch_add(n as u64, Ordering::SeqCst);
            let room = KEEP.saturating_sub(cap.head.len()).min(n);
            cap.head.extend_from_slice(&buf[..room]);
            cap.tail.extend(&buf[room..n]);
            while cap.tail.len() > CLIP_TAIL {
                cap.tail.pop_front();
            }
        }
        let _ = tx.send(cap);
    });
    rx
}

struct Done {
    out: String,
    err: String,
    status: Option<std::process::ExitStatus>,
    timed_out: bool,
    // Stopped for writing past its output limit.
    over: bool,
    // Stopped at its threads or written limit (proc::over_counts).
    on: Option<&'static str>,
}

thread_local! {
    // The limits spawn_capped checks the kernel's counts against while the program runs (run_bin sets them).
    static WATCH: std::cell::Cell<Option<crate::proc::Limits>> = const { std::cell::Cell::new(None) };
}

// Runs a command with stdin, a time limit and its output read as it comes. The child and everything it starts
// are killed when it ends, times out or the call is cancelled.
fn spawn(c: Command, stdin: &str, limit: Duration) -> Result<Done, String> {
    spawn_capped(c, stdin, limit, None)
}

// spawn, stopping the program once stdout and stderr together pass out_cap bytes (Done.over).
fn spawn_capped(mut c: Command, stdin: &str, limit: Duration, out_cap: Option<u64>) -> Result<Done, String> {
    if CANCELLED.load(Ordering::SeqCst) {
        return Err("cancelled".into());
    }
    c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    crate::proc::own_group(&mut c);
    crate::proc::die_with_parent(&mut c);
    let mut child = c.spawn().map_err(|e| format!("cannot start {}: {e}", c.get_program().to_string_lossy()))?;
    CHILD.store(child.id(), Ordering::SeqCst);
    // A cancel that came between the check above and the store: stop now.
    if CANCELLED.load(Ordering::SeqCst) {
        crate::proc::stop(&mut child);
    }
    if let Some(mut si) = child.stdin.take() {
        let s = stdin.as_bytes().to_vec();
        std::thread::spawn(move || {
            let _ = si.write_all(&s);
        });
    }
    let seen = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let out = reader(child.stdout.take().unwrap(), seen.clone());
    let err = reader(child.stderr.take().unwrap(), seen.clone());
    let start = Instant::now();
    let mut timed_out = false;
    let mut over = false;
    let mut on = None;
    let watch = WATCH.with(|w| w.take());
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if watch.as_ref().is_some_and(|l| {
                on = crate::proc::over_counts(child.id(), l, seen.load(Ordering::SeqCst));
                on.is_some()
            }) => {
                crate::proc::stop(&mut child);
                let _ = child.wait();
                break None;
            }
            Ok(None) if out_cap.is_some_and(|c| seen.load(Ordering::SeqCst) > c) => {
                over = true;
                crate::proc::stop(&mut child);
                let _ = child.wait();
                break None;
            }
            Ok(None) if start.elapsed() > limit || CANCELLED.load(Ordering::SeqCst) => {
                timed_out = !CANCELLED.load(Ordering::SeqCst);
                crate::proc::stop(&mut child);
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(3)),
            Err(_) => break None,
        }
    };
    // Anything the program left running (a background child holding the pipes) goes too.
    crate::proc::stop_pid(child.id());
    CHILD.store(0, Ordering::SeqCst);
    let get = |rx: mpsc::Receiver<Cap>| rx.recv_timeout(Duration::from_secs(2)).map(|c| c.text()).unwrap_or_default();
    let (out, err) = (get(out), get(err));
    if CANCELLED.load(Ordering::SeqCst) {
        return Err("cancelled".into());
    }
    let over = over || out_cap.is_some_and(|c| seen.load(Ordering::SeqCst) > c);
    Ok(Done { out, err, status, timed_out, over, on })
}

fn secs(d: Duration) -> String {
    format!("{} s", d.as_secs())
}

// Runs this same binary on the kept files: (text, failed, the main file afterwards: `brevi fix` rewrites it).
fn brevi_file(files: &mut Files, cmd: &str, extra: &[String], stdin: &str, limit: Duration) -> (String, bool, String) {
    let (t, err, after, _) = brevi_done(files, cmd, extra, stdin, limit);
    (t, err, after)
}

// The structuredContent fields of a plain run: the program's stdout and stderr apart (each kept up to KEEP
// bytes), its exit code (null when it was stopped by the timeout) and the milliseconds the call took.
fn run_fields(d: &Done, took: Duration) -> String {
    let keep = |s: &str| {
        let mut n = s.len().min(KEEP);
        while !s.is_char_boundary(n) {
            n -= 1;
        }
        s[..n].to_string()
    };
    let code = d.status.and_then(|s| s.code()).map_or("null".into(), |c| c.to_string());
    format!(
        r#""stdout":"{}","stderr":"{}","exit_code":{code},"timed_out":{},"ms":{}"#,
        esc(&keep(&d.out)),
        esc(&keep(&d.err)),
        d.timed_out,
        took.as_millis()
    )
}

// brevi_file, also giving back the finished process (None when it could not start).
fn brevi_done(files: &mut Files, cmd: &str, extra: &[String], stdin: &str, limit: Duration) -> (String, bool, String, Option<Done>) {
    wait_warm();
    let dir = match write_files(files) {
        Ok(d) => d,
        Err(m) => return (m, true, String::new(), None),
    };
    let main = files.main();
    files.built.insert(main.clone());
    let f = dir.join(&main);
    let exe = std::env::current_exe().unwrap_or_else(|_| "brevi".into());
    let mut c = Command::new(exe);
    c.arg(cmd).arg(&f).args(extra).current_dir(&dir);
    if cfg().mem_mb > 0 {
        c.env("BREVI_MEM_MB", cfg().mem_mb.to_string());
    }
    // Every build reports its path, typed too; the line is taken out of the text (take_path).
    c.env("BREVI_PATH", "1");
    let mut done = match spawn(c, stdin, limit) {
        Ok(d) => d,
        Err(m) => return (m, true, String::new(), None),
    };
    if let Some(p) = take_path(&mut done.err) {
        files.path = Some(p);
    }
    let after = std::fs::read_to_string(&f).unwrap_or_default();
    // Notes like {"line":3,"fixed":"totl -> total"} go to stderr; keep them, they matter to the model.
    let mut text = done.out.clone();
    // The CLI's --limit advice in the tool's own words.
    let err = done.err.replace("raise it with --limit ", "raise it in limits: ");
    if !err.trim().is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&err);
    }
    let Some(st) = done.status else {
        if done.timed_out {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&format!("{{\"e\":\"timeout\",\"msg\":\"stopped after {}; pass timeout (seconds, up to 600) for a longer run\"}}", secs(limit)));
        }
        return (text, true, after, Some(done));
    };
    if text.contains("rustc not found") {
        text.push_str("Brevi needs the Rust toolchain: install it from https://rustup.rs, then restart the MCP client.\n");
    }
    if !st.success() && text.trim().is_empty() {
        text = format!("failed with exit code {} and no output", st.code().map_or("?".into(), |c| c.to_string()));
    }
    (text, !st.success(), after, Some(done))
}

// Takes the first {"path":...} line out of a build's stderr. Brevi prints it before the program runs, so a
// line the program itself prints later is left alone.
fn take_path(err: &mut String) -> Option<String> {
    let mut at = 0;
    for l in err.split_inclusive('\n') {
        if l.starts_with("{\"path\":") {
            let line = l.trim_end().to_string();
            parse(&line)?;
            err.replace_range(at..at + l.len(), "");
            return Some(line);
        }
        at += l.len();
    }
    None
}

// The text and structuredContent of a tool result: the path line is also the text's last line when the
// program is off the typed path, for clients that ignore structured results.
fn with_path(text: String, path: Option<&str>, run: Option<String>) -> (String, String) {
    let Some(p) = path else {
        return match run {
            Some(r) => (text, format!(",\"structuredContent\":{{{r}}}")),
            None => (text, String::new()),
        };
    };
    let typed = parse(p).and_then(|j| j.get("path").and_then(|v| v.str()).map(|v| v == "typed")).unwrap_or(false);
    let text = if typed {
        text
    } else if text.is_empty() || text.ends_with('\n') {
        format!("{text}{p}")
    } else {
        format!("{text}\n{p}")
    };
    let structured = match run {
        Some(r) => format!("{},{r}}}", p.trim_end().strip_suffix('}').unwrap_or(p)),
        None => p.to_string(),
    };
    (text, format!(",\"structuredContent\":{structured}"))
}

fn brevi(files: &mut Files, cmd: &str, extra: &[String], stdin: &str, limit: Duration) -> (String, bool) {
    let (t, err, _) = brevi_file(files, cmd, extra, stdin, limit);
    (t, err)
}

fn call(files: &mut Files, name: &str, a: &J) -> (String, bool) {
    let s = |k: &str| a.get(k).and_then(|v| v.str()).unwrap_or("").to_string();
    let n = |k: &str| a.get(k).and_then(|v| v.num()).map(|x| x as i64);
    // Sandboxed unless the caller says otherwise: no file access or native code without an `allow` line.
    let sandboxed = a.get("sandbox").map_or(true, |v| v.truthy());
    let mut sandbox: Vec<String> = if sandboxed { vec!["--sandbox".into()] } else { vec![] };
    // What the caller grants the sandboxed program: an allow line in the code grants nothing under the sandbox.
    if sandboxed {
        for (k, flag) in [("allow", "--allow"), ("allow_py", "--allow-py"), ("allow_net", "--allow-net"), ("allow_read", "--allow-read"), ("allow_write", "--allow-write")] {
            let list: Vec<String> = a.get(k).map(|v| v.arr().iter().filter_map(|x| x.str().map(String::from)).collect()).unwrap_or_default();
            if list.is_empty() {
                continue;
            }
            let parsed = match k {
                "allow_net" => crate::caps::parse_net(&list.join(",")),
                // folders relative to where brevi mcp was started, passed on made absolute
                "allow_read" | "allow_write" => crate::caps::parse_paths(&k[6..], &list.join(",")),
                _ => crate::caps::parse_grant(k == "allow_py", &list.join(",")),
            };
            let value = match parsed {
                Err(m) => return (m.replace("--allow-py", "allow_py").replace("--allow-net", "allow_net").replace("--allow-read", "allow_read").replace("--allow-write", "allow_write").replace("--allow", "allow"), true),
                Ok(g) if k == "allow_read" || k == "allow_write" => g.iter().filter_map(|x| x.split_once(':').map(|(_, p)| p.to_string())).collect::<Vec<_>>().join(","),
                Ok(_) => list.join(","),
            };
            sandbox.extend([flag.to_string(), value]);
        }
    }
    let limit = Duration::from_secs(n("timeout").map_or(cfg().timeout, |t| t.clamp(1, 600) as u64));
    // The caller's resource limits on the program (validate checked each one).
    let mut lims = crate::proc::Limits::default();
    if let Some(J::Obj(kv)) = a.get("limits") {
        for (k, v) in kv {
            if let Some(t) = v.str() {
                let _ = lims.parse(&format!("{k}={t}"));
            }
        }
    }
    let code = a.get("code").and_then(|v| v.str()).map(String::from).filter(|_| name != "guide");
    let named = Some(s("file")).filter(|f| !f.is_empty());
    if let Some(f) = &named {
        if let Err(m) = file_ok(f) {
            return (m, true);
        }
    }
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
                let (t, err, after) = brevi_file(files, "fix", &flags, "", limit);
                let head = if t.trim().is_empty() && !err { "OK".to_string() } else { t.trim_end().to_string() };
                if !after.is_empty() && after != before {
                    files.all.insert(main, after);
                    return (format!("{head}\n(fixes applied to the kept program; show prints it)"), err);
                }
                return (head, err);
            }
            check(files, &flags, limit)
        }
        "run" => {
            let mut extra = sandbox.clone();
            extra.extend(lims.flags());
            let args: Vec<String> = a.get("args").map(|v| v.arr().iter().filter_map(|x| x.str().map(String::from)).collect()).unwrap_or_default();
            if !args.is_empty() {
                extra.push("--".to_string());
                extra.extend(args);
            }
            match a.get("cases").map(|c| c.arr().to_vec()).filter(|c| !c.is_empty()) {
                Some(cases) => run_cases(files, &extra, &cases, sandboxed, n("timeout").map(|t| Duration::from_secs(t.clamp(1, 600) as u64)), &lims),
                None => {
                    let t0 = Instant::now();
                    let (t, err, _, done) = brevi_done(files, "run", &extra, &s("stdin"), limit);
                    files.run = done.map(|d| run_fields(&d, t0.elapsed()));
                    (t, err)
                }
            }
        }
        "test" => brevi(files, "test", &[sandbox.clone(), lims.flags()].concat(), "", limit),
        "verify" => verify(files, a, sandboxed, limit, &lims),
        "patch" => {
            let Some(cur) = files.all.get(&file).cloned() else { return (format!("no file {file}"), true) };
            match crate::patch::patch(&cur, &s("patch")) {
                Ok((out, done)) => {
                    files.all.insert(file, out);
                    then_check(files, &sandbox, &done.join(", "), limit)
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
            then_check(files, &sandbox, &format!("lines {from}..{to} now {added} line{}", if added == 1 { "" } else { "s" }), limit)
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

// `brevi verify` on the kept program: each input is written next to it and fed to both runs. CPython runs the
// code, so the sandbox (on by default) refuses it rather than run it unheld.
fn verify(files: &mut Files, a: &J, sandboxed: bool, limit: Duration, lims: &crate::proc::Limits) -> (String, bool) {
    if sandboxed {
        return ("verify runs python3 on the code, which the sandbox cannot hold to its grants: pass sandbox false to verify code you would run with python3".into(), true);
    }
    if !files.main().ends_with(".py") {
        return ("verify compares plain Python with CPython; Brevi code has no CPython answer to compare with (use test)".into(), true);
    }
    let strs = |k: &str| -> Vec<String> { a.get(k).map(|v| v.arr().iter().filter_map(|x| x.str().map(String::from)).collect()).unwrap_or_default() };
    let inputs = strs("inputs");
    let mut extra: Vec<String> = vec!["--timeout".into(), limit.as_secs().max(1).to_string()];
    extra.extend(lims.flags());
    let once = a.get("once").map_or(false, |v| v.truthy());
    if once {
        extra.push("--once".into());
    }
    // Next to the work directory, which every call writes afresh.
    let dir = PathBuf::from(format!("{}-verify", work_dir().display()));
    let _ = std::fs::remove_dir_all(&dir);
    if !inputs.is_empty() {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return (format!("cannot write the inputs: {e}"), true);
        }
        for (i, text) in inputs.iter().enumerate() {
            let f = dir.join(format!("input{}", i + 1));
            if let Err(e) = std::fs::write(&f, text) {
                return (format!("cannot write the inputs: {e}"), true);
            }
            extra.extend(["--input".to_string(), f.to_string_lossy().to_string()]);
        }
    }
    let args = strs("args");
    if !args.is_empty() {
        extra.push("--".into());
        extra.extend(args);
    }
    // The build, then CPython twice and the build once per input, each within the limit.
    let runs = if once { 2 } else { 3 } * inputs.len().max(1) as u32 + 1;
    let total = Duration::from_secs(cfg().timeout).max(limit) + limit * runs;
    let (t, err) = brevi(files, "verify", &extra, "", total.min(Duration::from_secs(1800)));
    let _ = std::fs::remove_dir_all(&dir);
    // Input paths are the server's own temporary files: name them input1, input2, ... in the reply.
    let shown = esc(&(dir.to_string_lossy().to_string() + std::path::MAIN_SEPARATOR_STR));
    let work = esc(&(work_dir().to_string_lossy().to_string() + std::path::MAIN_SEPARATOR_STR));
    // The CLI's advice in the tool's own words.
    let t = t.replace(&shown, "").replace(&work, "").replace("pass --once", "pass once true").replace("pass --timeout SECS", "pass timeout (seconds)").replace("raise it with --limit ", "raise it in limits: ");
    (t, err)
}

fn check(files: &mut Files, flags: &[String], limit: Duration) -> (String, bool) {
    let (t, err) = brevi(files, "check", flags, "", limit);
    (if t.trim().is_empty() && !err { "OK".into() } else { t }, err)
}

// After a change: what changed, then OK or the errors, in one reply.
fn then_check(files: &mut Files, flags: &[String], what: &str, limit: Duration) -> (String, bool) {
    let (t, err) = check(files, flags, limit);
    (format!("{what}\n{}", t.trim_end()), err)
}

// Builds once, then runs each case. Prints ok per case, or the first line that differs. Plain Python that
// only CPython can run (sandbox off) runs each case through `brevi run`, as run without cases would.
fn run_cases(files: &mut Files, extra: &[String], cases: &[J], sandboxed: bool, per_case: Option<Duration>, lims: &crate::proc::Limits) -> (String, bool) {
    // --limit holds the runs, not the build.
    let mut flags: Vec<String> = extra.iter().take_while(|a| *a != "--").cloned().collect();
    if let Some(i) = flags.iter().position(|f| f == "--limit") {
        flags.drain(i..(i + 2).min(flags.len()));
    }
    let args: Vec<String> = extra.iter().skip_while(|a| *a != "--").skip(1).cloned().collect();
    let bin_name = ".brevi-cases-bin";
    let bin = work_dir().join(bin_name);
    flags.extend(["-o".to_string(), bin.to_string_lossy().to_string()]);
    let per_case = per_case.unwrap_or(Duration::from_secs(10));
    let (t, err) = brevi(files, "build", &flags, "", Duration::from_secs(cfg().timeout).max(per_case));
    let mut via_run = false;
    if err {
        if sandboxed || !files.main().ends_with(".py") {
            return (t, true);
        }
        via_run = true;
    }
    let mut out = vec![];
    let mut failed = 0;
    let mut run_flags = extra.to_vec();
    if !run_flags.iter().any(|a| a == "--") {
        run_flags.push("--".into());
    }
    for (i, c) in cases.iter().enumerate() {
        let stdin = c.get("stdin").and_then(|v| v.str()).unwrap_or("");
        let want = c.get("expect").and_then(|v| v.str()).unwrap_or("");
        let (got, code) = if via_run {
            let (got, failed) = brevi(files, "run", extra, stdin, per_case + Duration::from_secs(cfg().timeout));
            // Not a fallback: the build error is the answer, as for run without cases.
            // (The {"path":"cpython"} line is already out of got, in files.path.)
            if i == 0 && !files.path.as_deref().is_some_and(|p| p.starts_with("{\"path\":\"cpython\"")) {
                return (got, true);
            }
            (got, failed)
        } else {
            run_bin(&bin, &args, stdin, per_case, lims)
        };
        let (gl, wl): (Vec<&str>, Vec<&str>) = (got.trim_end().lines().map(str::trim_end).collect(), want.trim_end().lines().map(str::trim_end).collect());
        // Stopped by one of the caller's limits: that is the answer, not a line that differs.
        if let Some(l) = gl.iter().rev().find(|l| l.starts_with("{\"e\":\"limit\"")).filter(|_| code) {
            failed += 1;
            out.push(format!("case {}: {l}", i + 1));
            continue;
        }
        if gl == wl && !code {
            out.push(format!("case {}: ok", i + 1));
            continue;
        }
        failed += 1;
        if gl == wl {
            out.push(format!("case {}: output matches but the program failed", i + 1));
            continue;
        }
        let k = gl.iter().zip(&wl).take_while(|(g, w)| g == w).count();
        let g = gl.get(k).map_or("(no more output)".to_string(), |l| format!("{l:?}"));
        let w = wl.get(k).map_or("(no more output)".to_string(), |l| format!("{l:?}"));
        let crash = if code { " (program failed)" } else { "" };
        out.push(format!("case {}: line {} want {w} got {g}{crash}", i + 1, k + 1));
        // The error itself, when the program failed: the model needs it to fix the program.
        if code {
            if let Some(e) = gl.iter().rev().find(|l| l.starts_with("{\"e\"") || l.starts_with("{\"line\"") || l.contains("Error")) {
                if Some(e) != gl.get(k) {
                    out.push(format!("  {e}"));
                }
            }
        }
    }
    let _ = std::fs::remove_file(&bin);
    if via_run {
        out.push("note: ran with python3 (Brevi could not build this program)".into());
    }
    out.push(format!("passed {}/{}", cases.len() - failed, cases.len()));
    (out.join("\n"), failed > 0)
}

// Runs a built program with a time limit and the call's resource limits; stdout plus any error line, and
// whether it failed.
fn run_bin(bin: &Path, args: &[String], stdin: &str, limit: Duration, lims: &crate::proc::Limits) -> (String, bool) {
    let mut c = crate::proc::program(bin);
    c.args(args).current_dir(work_dir());
    crate::proc::apply(&mut c, lims);
    let wall = lims.time.map_or(limit, |t| t.min(limit));
    WATCH.with(|w| w.set(Some(*lims)));
    let d = match spawn_capped(c, stdin, wall, lims.output) {
        Ok(d) => d,
        Err(m) => return (m, true),
    };
    let mut t = d.out;
    if let Some(o) = lims.output {
        // What the program wrote up to its limit, as `brevi run --limit output=...` passes on.
        let mut cut = (o as usize).min(t.len());
        while !t.is_char_boundary(cut) {
            cut -= 1;
        }
        t.truncate(cut);
    }
    let hit = if d.on.is_some() {
        d.on
    } else if d.over {
        Some("output")
    } else if d.timed_out && lims.time.is_some_and(|x| x <= limit) {
        Some("time")
    } else {
        d.status.and_then(|s| crate::proc::limit_hit(lims, s, &d.err))
    };
    let failed = hit.is_some() || !d.status.is_some_and(|s| s.success());
    if failed && hit.is_none() && !d.err.trim().is_empty() {
        if !t.is_empty() && !t.ends_with('\n') {
            t.push('\n');
        }
        t.push_str(d.err.trim_end());
    }
    if let Some(on) = hit {
        if !t.is_empty() && !t.ends_with('\n') {
            t.push('\n');
        }
        t.push_str(&crate::proc::limit_line(lims, on).replace("raise it with --limit ", "raise it in limits: "));
    } else if d.timed_out {
        t.push_str(&format!("\n{{\"e\":\"timeout\",\"msg\":\"stopped after {}\"}}", secs(limit)));
    }
    (t, failed)
}

// Long text keeps its start and end, with a note in the middle.
fn clip(t: &str) -> String {
    if t.len() <= CLIP_HEAD + CLIP_TAIL + 200 {
        return t.to_string();
    }
    let mut h = CLIP_HEAD;
    while !t.is_char_boundary(h) {
        h -= 1;
    }
    let mut s = t.len() - CLIP_TAIL;
    while !t.is_char_boundary(s) {
        s += 1;
    }
    let cut = &t[h..s];
    format!("{}\n[... {} lines, {} bytes cut ...]\n{}", &t[..h], cut.matches('\n').count(), cut.len(), &t[s..])
}

// One reply line on stdout. If the client has gone, there is nothing left to do.
fn send(line: &str) {
    static OUT: Mutex<()> = Mutex::new(());
    let _g = OUT.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = std::io::stdout().lock();
    if writeln!(out, "{line}").and_then(|_| out.flush()).is_err() {
        shutdown(1);
    }
}

fn reply(id: &str, result: Result<String, (i32, String)>) {
    match result {
        Ok(r) => send(&format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{r}}}"#)),
        Err((code, m)) => send(&format!(r#"{{"jsonrpc":"2.0","id":{id},"error":{{"code":{code},"message":"{}"}}}}"#, esc(&m))),
    }
}

// Removes this server's work directory and the build directories of the files it built.
fn clean(built: &BTreeSet<String>) {
    let w = work_dir();
    if let Ok(real) = std::fs::canonicalize(w) {
        for name in built {
            let _ = std::fs::remove_dir_all(crate::build_dir_path(&real.join(name).to_string_lossy()));
        }
    }
    let _ = std::fs::remove_dir_all(w);
    let warm = warm_dir();
    if let Ok(real) = std::fs::canonicalize(&warm) {
        let _ = std::fs::remove_dir_all(crate::build_dir_path(&real.join("warm.py").to_string_lossy()));
    }
    let _ = std::fs::remove_dir_all(warm);
}

// Work directories of servers that are no longer running.
fn clean_stale() {
    let Ok(rd) = std::fs::read_dir(std::env::temp_dir()) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(rest) = name.strip_prefix("brevi-mcp-") else { continue };
        let Ok(pid) = rest.split('-').next().unwrap_or("").parse::<u32>() else { continue };
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|a| a > Duration::from_secs(7 * 86400));
        if pid == std::process::id() || (crate::proc::alive(pid) && !old) {
            continue;
        }
        if let Ok(real) = std::fs::canonicalize(e.path()) {
            if let Ok(inner) = std::fs::read_dir(&real) {
                for f in inner.flatten() {
                    let _ = std::fs::remove_dir_all(crate::build_dir_path(&f.path().to_string_lossy()));
                }
            }
        }
        let _ = std::fs::remove_dir_all(e.path());
    }
}

static BUILT: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

fn shutdown(code: i32) -> ! {
    SHUTTING.store(true, Ordering::SeqCst);
    CANCELLED.store(true, Ordering::SeqCst);
    let pid = CHILD.load(Ordering::SeqCst);
    if pid != 0 {
        crate::proc::stop_pid(pid);
    }
    clean(&BUILT.lock().unwrap_or_else(|e| e.into_inner()));
    std::process::exit(code)
}

struct Job {
    id: String,
    name: String,
    args: J,
    modern: bool,
}

pub fn serve(argv: &[String]) {
    if argv.iter().any(|a| a == "--http") {
        crate::mcp_http::serve(argv);
    }
    let mut c = Cfg { timeout: 20, mem_mb: default_mem_mb() };
    let mut i = 0;
    while i < argv.len() {
        let v = argv.get(i + 1).and_then(|v| v.parse::<u64>().ok());
        match (argv[i].as_str(), v) {
            ("--timeout", Some(t)) => c.timeout = t.clamp(1, 600),
            ("--mem-mb", Some(m)) => c.mem_mb = m,
            _ => {
                eprintln!("usage: brevi mcp [--timeout SECONDS] [--mem-mb MB (0: no limit)] [--http [HOST:]PORT [--token TOKEN] [--path /mcp] [--allow-origin ORIGIN]]");
                std::process::exit(2);
            }
        }
        i += 2;
    }
    let _ = CFG.set(c);
    find_rust();
    clean_stale();
    std::thread::spawn(warm_up);

    // Tool calls run in order on one worker; the ids of calls cancelled before they started are skipped.
    let skip: Arc<Mutex<HashSet<String>>> = Arc::default();
    let current: Arc<Mutex<Option<String>>> = Arc::default();
    let (tx, rx) = mpsc::channel::<Job>();
    let worker = {
        let (skip, current) = (skip.clone(), current.clone());
        std::thread::spawn(move || {
            let mut files = Files::default();
            for job in rx {
                {
                    let mut cur = current.lock().unwrap();
                    if skip.lock().unwrap().remove(&job.id) || SHUTTING.load(Ordering::SeqCst) {
                        continue;
                    }
                    *cur = Some(job.id.clone());
                    CANCELLED.store(false, Ordering::SeqCst);
                }
                files.path = None;
                files.run = None;
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call(&mut files, &job.name, &job.args)));
                BUILT.lock().unwrap_or_else(|e| e.into_inner()).extend(files.built.iter().cloned());
                let (text, err) = r.unwrap_or_else(|p| {
                    let m = p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_default();
                    (format!("internal error in brevi mcp ({m}); the kept program is unchanged. Please report it."), true)
                });
                let cancelled = {
                    let mut cur = current.lock().unwrap();
                    *cur = None;
                    CANCELLED.swap(false, Ordering::SeqCst)
                };
                // A cancelled request gets no reply (the MCP spec's rule).
                if !cancelled {
                    let (text, structured) = with_path(clip(&text), files.path.take().as_deref(), files.run.take());
                    let r = format!(r#"{{"content":[{{"type":"text","text":"{}"}}]{structured},"isError":{err}}}"#, esc(&text));
                    reply(&job.id, Ok(if job.modern { stamp(&r, false) } else { r }));
                }
            }
        })
    };

    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match input.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let Ok(line) = std::str::from_utf8(&buf) else {
            reply("null", Err((-32700, "parse error: the message is not UTF-8".into())));
            continue;
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some(msg) = parse(line) else {
            reply("null", Err((-32700, "parse error: not valid JSON".into())));
            continue;
        };
        let J::Obj(_) = msg else {
            let m = if matches!(msg, J::Arr(_)) { "batches are not supported: send one request per line" } else { "a request must be a JSON object" };
            reply("null", Err((-32600, m.into())));
            continue;
        };
        let id = match msg.get("id") {
            Some(v @ (J::Str(_) | J::Num(_))) => Some(to_json(v)),
            Some(J::Null) | None => None,
            Some(_) => {
                reply("null", Err((-32600, "id must be a string or a number".into())));
                continue;
            }
        };
        let method = match msg.get("method") {
            Some(J::Str(m)) => m.clone(),
            // A reply to a request: this server sends none, so there is nothing to match it with.
            None if msg.get("result").is_some() || msg.get("error").is_some() => continue,
            _ => {
                if let Some(id) = id {
                    reply(&id, Err((-32600, "a request needs a method".into())));
                }
                continue;
            }
        };
        let params = msg.get("params").cloned().unwrap_or(J::Obj(vec![]));
        let Some(id) = id else {
            // A notification: no reply. Cancelling stops the named call, or skips it if it has not started.
            if method == "notifications/cancelled" {
                if let Some(rid) = params.get("requestId").filter(|v| matches!(v, J::Str(_) | J::Num(_))).map(to_json) {
                    let cur = current.lock().unwrap();
                    if cur.as_deref() == Some(rid.as_str()) {
                        CANCELLED.store(true, Ordering::SeqCst);
                        let pid = CHILD.load(Ordering::SeqCst);
                        if pid != 0 {
                            crate::proc::stop_pid(pid);
                        }
                    } else {
                        skip.lock().unwrap().insert(rid);
                    }
                }
            }
            continue;
        };
        if !matches!(params, J::Obj(_)) {
            reply(&id, Err((-32602, "params must be an object".into())));
            continue;
        }
        let modern = match envelope(&params) {
            Ok(m) => m,
            Err(e) => {
                send(&format!(r#"{{"jsonrpc":"2.0","id":{id},"error":{e}}}"#));
                continue;
            }
        };
        let done = |r: String, cached: bool| if modern { stamp(&r, cached) } else { r };
        match method.as_str() {
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(|v| v.str()).unwrap_or("");
                let v = VERSIONS.iter().find(|v| **v == asked).unwrap_or(&VERSIONS[0]);
                reply(&id, Ok(format!(r#"{{"protocolVersion":"{v}","capabilities":{{"tools":{{"listChanged":false}}}},"serverInfo":{{"name":"brevi","title":"Brevi","version":"{}"}},"instructions":"{}"}}"#, env!("CARGO_PKG_VERSION"), esc(INSTRUCTIONS))));
            }
            "server/discover" if !modern => send(&format!(
                r#"{{"jsonrpc":"2.0","id":{id},"error":{{"code":-32602,"message":"params._meta must be an object carrying the required '{K_VERSION}' and '{K_CAPS}' envelope keys"}}}}"#
            )),
            "server/discover" => reply(
                &id,
                Ok(stamp(&format!(r#"{{"supportedVersions":["{}"],"capabilities":{{"tools":{{"listChanged":false}}}},"instructions":"{}"}}"#, MODERN.join("\",\""), esc(INSTRUCTIONS)), true)),
            ),
            "ping" => reply(&id, Ok(done("{}".into(), false))),
            "tools/list" => reply(&id, Ok(done(format!(r#"{{"tools":{}}}"#, TOOLS.replace('\n', "")), true))),
            "tools/call" => {
                let Some(name) = params.get("name").and_then(|v| v.str()) else {
                    reply(&id, Err((-32602, "tools/call needs params.name".into())));
                    continue;
                };
                if spec(name).is_none() {
                    reply(&id, Err((-32602, format!("unknown tool {name:?}; the tools are guide, check, run, test, patch, edit, show"))));
                    continue;
                }
                match validate(name, &params.get("arguments").cloned().unwrap_or(J::Null)) {
                    // A wrong argument is a tool error, so the model sees it and can correct the call.
                    Err(m) => reply(&id, Ok(done(format!(r#"{{"content":[{{"type":"text","text":"{}"}}],"isError":true}}"#, esc(&m)), false))),
                    Ok(args) => {
                        let _ = tx.send(Job { id, name: name.to_string(), args, modern });
                    }
                }
            }
            m if m.starts_with("notifications/") => {}
            _ => reply(&id, Err((-32601, format!("method not found: {method}")))),
        }
    }
    // The client closed stdin: stop what is running, drop what is queued (no one will read the replies),
    // clean up.
    {
        let _cur = current.lock().unwrap();
        SHUTTING.store(true, Ordering::SeqCst);
        CANCELLED.store(true, Ordering::SeqCst);
        let pid = CHILD.load(Ordering::SeqCst);
        if pid != 0 {
            crate::proc::stop_pid(pid);
        }
    }
    drop(tx);
    let _ = worker.join();
    shutdown(0);
}

// MCP clients often start servers with a short PATH (Claude Desktop on macOS: /usr/bin:/bin:/usr/sbin:/sbin),
// which leaves out ~/.cargo/bin where rustup puts rustc (and install.sh puts brevi). Add it when rustc is
// not found otherwise. Runs before any thread starts.
fn find_rust() {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let exe = if cfg!(windows) { "rustc.exe" } else { "rustc" };
    if std::env::split_paths(&path).any(|d| d.join(exe).is_file()) {
        return;
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let cargo = std::env::var_os("CARGO_HOME").map(PathBuf::from).or_else(|| home.map(|h| h.join(".cargo")));
    let Some(bin) = cargo.map(|c| c.join("bin")).filter(|b| b.join(exe).is_file()) else { return };
    let mut dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
    dirs.insert(0, bin);
    if let Ok(p) = std::env::join_paths(dirs) {
        // Safe: no other thread exists yet.
        unsafe { std::env::set_var("PATH", p) };
    }
}

// Half the machine's memory (at least 2 GB) on Linux; elsewhere no limit.
fn default_mem_mb() -> u64 {
    let total_kb = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|m| m.lines().find(|l| l.starts_with("MemTotal:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()));
    total_kb.map_or(0, |kb| (kb / 1024 / 2).max(2048))
}
