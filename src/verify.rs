// `brevi verify FILE.py`: proof that the native build prints what CPython prints.
//
// Builds the file natively (strict, as `brevi build --native`), then runs CPython and the built program on
// the same inputs and compares stdout, the exit code and stderr (CPython's traceback frames left out: a built
// program names the exception, not the frames). CPython runs each input twice, so output that changes from
// run to run (set order, time, unseeded random) is reported as unstable rather than certified.
//
//   brevi verify FILE.py [--input PATH]... [--once] [--mcp] [--timeout SECS] [--release] [--no-cache] [-- ARGS...]
//
// --input PATH is fed to the program's stdin, one run per path; a folder gives one run per file in it (only
// its *.in files when it has any). With no input the program runs once with empty stdin. ARGS go to both.
// For an MCP server (a file that imports mcp) a .jsonl input, or any input with --mcp, is a JSON-RPC session:
// each request goes in once the reply to the one before it is out, as a client sends them (see lockstep).
//
// One JSON line per input, then the certificate:
//   {"case":1,"input":"t/1.in","same":true,"cpython_ms":38,"brevi_ms":2}
//   {"verify":"identical","file":"a.py","sha256":"...","inputs":[...],"path":"typed",...}
// The verdict is identical, differs (a wrong answer from Brevi), unstable (CPython's own answer changed between
// its two runs), timeout, or not built (Brevi refused the file natively, so nothing was compared).
// Exit 0 when every input matched, 1 when one differed, was unstable or timed out, or the file did not build,
// 2 for a usage error. --sandbox is refused: verify runs the program under CPython, which cannot hold it.
use crate::diag::esc;
use crate::dynpy::tok::{hex, sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{exit, Stdio};
use std::time::{Duration, Instant};

const USAGE: &str = "usage: brevi verify FILE.py [--input PATH]... [--once] [--mcp] [--timeout SECS] [--release] [--no-cache] [-- ARGS...]";

fn usage(msg: &str) -> ! {
    println!("{{\"verify\":\"error\",\"msg\":\"{}\",\"fix\":\"{}\"}}", esc(msg), esc(USAGE));
    exit(2);
}

struct Run {
    out: Vec<u8>,
    err: Vec<u8>,
    // None: killed by a signal, or stopped at the time limit (timed_out).
    code: Option<i32>,
    ms: u128,
    timed_out: bool,
}

fn run(mut c: std::process::Command, stdin: &[u8], limit: Duration, session: bool) -> Result<Run, String> {
    c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    crate::proc::own_group(&mut c);
    let start = Instant::now();
    let mut child = c.spawn().map_err(|e| format!("cannot start {}: {e}", c.get_program().to_string_lossy()))?;
    let si = child.stdin.take();
    let input = stdin.to_vec();
    let read = |r: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut b = vec![];
            if let Some(mut r) = r {
                let _ = r.read_to_end(&mut b);
            }
            b
        })
    };
    let out = if session {
        // a JSON-RPC session (an MCP server): each line goes in once the reply to the one before it is out
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let so = child.stdout.take();
        let out = std::thread::spawn(move || {
            let mut all = vec![];
            if let Some(so) = so {
                let mut r = std::io::BufReader::new(so);
                loop {
                    let mut l = vec![];
                    match std::io::BufRead::read_until(&mut r, b'\n', &mut l) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            all.extend_from_slice(&l);
                            let _ = tx.send(String::from_utf8_lossy(&l).trim_end().to_string());
                        }
                    }
                }
            }
            all
        });
        std::thread::spawn(move || {
            if let Some(si) = si {
                lockstep(si, &input, &rx, limit);
            }
        });
        out
    } else {
        std::thread::spawn(move || {
            if let Some(mut si) = si {
                let _ = si.write_all(&input);
            }
        });
        read(child.stdout.take().map(|r| Box::new(r) as Box<dyn Read + Send>))
    };
    let err = read(child.stderr.take().map(|r| Box::new(r) as Box<dyn Read + Send>));
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if start.elapsed() > limit => {
                timed_out = true;
                crate::proc::stop(&mut child);
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_micros(500)),
            Err(_) => break None,
        }
    };
    let ms = start.elapsed().as_millis();
    // Anything the program left running (a background child holding the pipes) goes too.
    crate::proc::stop_pid(child.id());
    Ok(Run { out: out.join().unwrap_or_default(), err: err.join().unwrap_or_default(), code: status.and_then(|s| exit_code(s)), ms, timed_out })
}

// The id a JSON-RPC message answers or asks with, as Python compares them (1 == 1.0, "1" != 1); None for none.
fn rpc_id(j: &crate::json::J) -> Option<String> {
    match j.get("id")? {
        crate::json::J::Num(n) => Some(format!("n{n}")),
        crate::json::J::Str(t) => Some(format!("s{t}")),
        crate::json::J::Bool(b) => Some(format!("n{}", *b as u8)),
        _ => None,
    }
}

// Plays a session script (tests/mcp_sdk/drive.py's): one message per line, sent as written; a request (a
// method and an id) waits for the reply with its id. `#nowait MSG` sends without waiting, `#wait N` waits for N
// more lines, `#raw TEXT` sends TEXT and waits up to 2 s for a line, `#close` ends the session, `# ...` is a
// comment. stdin closes at the end, as a client hanging up.
fn lockstep(mut si: std::process::ChildStdin, script: &[u8], rx: &std::sync::mpsc::Receiver<String>, limit: Duration) {
    let text = String::from_utf8_lossy(script).to_string();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let send = |si: &mut std::process::ChildStdin, m: &str| si.write_all(format!("{m}\n").as_bytes()).and_then(|_| si.flush()).is_ok();
        if let Some(m) = line.strip_prefix("#raw ") {
            if !send(&mut si, m) {
                return;
            }
            let _ = rx.recv_timeout(Duration::from_secs(2));
            continue;
        }
        if let Some(n) = line.strip_prefix("#wait ") {
            for _ in 0..n.trim().parse::<usize>().unwrap_or(0) {
                if rx.recv_timeout(limit).is_err() {
                    return;
                }
            }
            continue;
        }
        if let Some(m) = line.strip_prefix("#nowait ") {
            if !send(&mut si, m) {
                return;
            }
            continue;
        }
        if line.starts_with("#close") {
            return;
        }
        if line.starts_with('#') {
            continue;
        }
        if !send(&mut si, line) {
            return;
        }
        let Some(msg) = crate::json::parse(t) else { continue };
        let Some(id) = rpc_id(&msg) else { continue };
        if msg.get("method").is_none() {
            continue;
        }
        loop {
            match rx.recv_timeout(limit) {
                Ok(l) => {
                    if let Some(r) = crate::json::parse(&l) {
                        if r.get("method").is_none() && rpc_id(&r).as_deref() == Some(id.as_str()) {
                            break;
                        }
                    }
                }
                Err(_) => return,
            }
        }
    }
}

fn exit_code(s: std::process::ExitStatus) -> Option<i32> {
    if let Some(c) = s.code() {
        return Some(c);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        return s.signal().map(|g| 128 + g);
    }
    #[allow(unreachable_code)]
    None
}

// stderr without CPython's traceback frames: after "Traceback (most recent call last):", the indented lines
// (File ..., the source line, ~~~^^^ markers) are left out; the header and the exception line stay.
fn norm_err(b: &[u8]) -> Vec<String> {
    let mut out = vec![];
    let mut in_tb = false;
    for l in String::from_utf8_lossy(b).lines() {
        let l = l.trim_end();
        if l == "Traceback (most recent call last):" {
            in_tb = true;
            out.push(l.to_string());
            continue;
        }
        if in_tb && (l.starts_with(' ') || l.starts_with('\t')) {
            continue;
        }
        in_tb = false;
        out.push(l.to_string());
    }
    out
}

// The first line where two outputs differ: (1-based line, CPython's line, Brevi's line).
fn first_diff(a: &[u8], b: &[u8]) -> (usize, String, String) {
    let (a, b) = (String::from_utf8_lossy(a), String::from_utf8_lossy(b));
    let al: Vec<&str> = a.split_inclusive('\n').collect();
    let bl: Vec<&str> = b.split_inclusive('\n').collect();
    let k = al.iter().zip(&bl).take_while(|(x, y)| x == y).count();
    let show = |v: &Vec<&str>| v.get(k).map_or("(no more output)".to_string(), |l| clip(l));
    (k + 1, show(&al), show(&bl))
}

fn clip(l: &str) -> String {
    let l = l.strip_suffix('\n').map_or_else(|| format!("{l}(no newline)"), |s| s.to_string());
    if l.chars().count() > 200 {
        format!("{}...", l.chars().take(200).collect::<String>())
    } else {
        l
    }
}

fn inputs(paths: &[String]) -> Vec<(String, Vec<u8>)> {
    let mut out = vec![];
    for p in paths {
        let path = Path::new(p);
        if path.is_dir() {
            let mut files: Vec<PathBuf> = std::fs::read_dir(path)
                .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|f| f.is_file() && !f.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'))).collect())
                .unwrap_or_default();
            if files.iter().any(|f| f.extension().is_some_and(|e| e == "in")) {
                files.retain(|f| f.extension().is_some_and(|e| e == "in"));
            }
            files.sort();
            if files.is_empty() {
                usage(&format!("no input files in {p}"));
            }
            for f in files {
                let name = f.to_string_lossy().to_string();
                let data = std::fs::read(&f).unwrap_or_else(|e| usage(&format!("cannot read {name}: {e}")));
                out.push((name, data));
            }
        } else {
            let data = std::fs::read(path).unwrap_or_else(|e| usage(&format!("cannot read {p}: {e}")));
            out.push((p.clone(), data));
        }
    }
    if out.is_empty() {
        out.push(("(empty stdin)".to_string(), vec![]));
    }
    out
}

fn python_version(python: &str) -> String {
    let o = crate::proc::program(python).arg("-c").arg("import sys; print('%d.%d.%d' % sys.version_info[:3])").output();
    match o {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => {
            println!("{{\"verify\":\"error\",\"msg\":\"{} not found: verify compares the build with CPython\",\"fix\":\"install python3, or set BREVI_PYTHON to the interpreter to compare with\"}}", esc(python));
            exit(2);
        }
    }
}

pub fn main(argv: &[String]) -> ! {
    let dash = argv.iter().position(|a| a == "--").unwrap_or(argv.len());
    let prog_args: Vec<String> = argv.get(dash + 1..).unwrap_or(&[]).to_vec();
    let mut file = None;
    let mut input_paths: Vec<String> = vec![];
    let mut once = false;
    let mut release = false;
    let mut mcp = false;
    let mut limit = Duration::from_secs(60);
    let mut i = 0;
    while i < dash {
        let a = argv[i].as_str();
        let value = |i: usize| argv.get(i + 1).filter(|_| i + 1 < dash).cloned().unwrap_or_else(|| usage(&format!("{a} needs a value")));
        match a {
            "--input" => {
                input_paths.push(value(i));
                i += 1;
            }
            _ if a.starts_with("--input=") => input_paths.push(a["--input=".len()..].to_string()),
            "--timeout" => {
                let v = value(i);
                limit = Duration::from_secs(v.parse::<u64>().ok().filter(|s| *s > 0).unwrap_or_else(|| usage("--timeout takes whole seconds above 0")));
                i += 1;
            }
            "--once" => once = true,
            "--mcp" => mcp = true,
            "--release" => release = true,
            "--no-cache" => crate::runcache::turn_off(),
            _ if a == "--sandbox" || a.starts_with("--allow") => usage(
                "verify runs the program under CPython, which the sandbox cannot hold to its grants, so verify does not take --sandbox or --allow; verify only code you would run with python3",
            ),
            _ if a.starts_with('-') => usage(&format!("verify has no option {a}")),
            _ if file.is_none() => file = Some(a.to_string()),
            _ => usage(&format!("one file at a time ({a} is a second one); pass program arguments after --")),
        }
        i += 1;
    }
    let Some(file) = file else { usage("verify needs a .py file") };
    if !file.ends_with(".py") {
        usage("verify compares a .py file with CPython; a .bv file has no CPython answer to compare with (use brevi test)");
    }
    let src = std::fs::read(&file).unwrap_or_else(|e| usage(&format!("cannot read {file}: {e}")));
    let cases = inputs(&input_paths);
    let serves_mcp = String::from_utf8_lossy(&src).lines().any(|l| {
        let l = l.trim_start();
        l.starts_with("from mcp") || l.starts_with("import mcp")
    });
    let python = std::env::var("BREVI_PYTHON").unwrap_or_else(|_| "python3".into());
    let pyver = python_version(&python);

    // The native build, strict: no CPython fallback, so what runs below is Brevi's own code.
    let stem = Path::new(&file).file_stem().unwrap().to_string_lossy().to_string();
    let bin = crate::build_dir_path(&file).join(format!("{stem}-verify"));
    let mut args: Vec<String> = vec!["build".into(), file.clone(), "-o".into(), bin.to_string_lossy().to_string()];
    if release {
        args.push("--release".into());
    }
    let envs = [("BREVI_NATIVE", "1"), ("BREVI_PATH", "1")];
    let built = if crate::runcache::on() {
        crate::runcache::phase(&args, &file, Some(&bin), &envs, true)
    } else {
        let me = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("brevi"));
        let mut c = std::process::Command::new(me);
        c.args(&args).envs(envs).env("BREVI_NO_CACHE", "1");
        match c.output() {
            Ok(o) => crate::runcache::Done { stdout: o.stdout, stderr: o.stderr, code: o.status.code(), hit: false },
            Err(e) => usage(&format!("cannot start brevi build: {e}")),
        }
    };
    let err = String::from_utf8_lossy(&built.stderr).to_string();
    let path_line = err.lines().find(|l| l.starts_with("{\"path\":")).map(String::from);
    let path = path_line.as_deref().and_then(crate::json::parse).and_then(|j| j.get("path").and_then(|v| v.str()).map(String::from)).unwrap_or_else(|| "typed".into());
    let common = format!(
        "\"file\":\"{}\",\"sha256\":\"{}\",\"brevi\":\"{}\",\"python\":\"{}\"",
        esc(&file),
        hex(&sha256(&src)),
        env!("CARGO_PKG_VERSION"),
        esc(&pyver)
    );
    if !built.ok() {
        // Brevi's own errors (one JSON line each), then the verdict: nothing was compared.
        print!("{}", String::from_utf8_lossy(&built.stdout));
        err.lines().filter(|l| !l.starts_with("{\"path\":")).for_each(|l| println!("{l}"));
        println!("{{\"verify\":\"not built\",{common},\"msg\":\"Brevi could not build this file natively, so there is nothing to compare; brevi run would hand it to CPython\"}}");
        exit(1);
    }

    let (mut differ, mut unstable, mut timeouts) = (0, 0, 0);
    let (mut py_ms, mut bv_ms) = (0u128, 0u128);
    let mut ins = vec![];
    for (n, (name, data)) in cases.iter().enumerate() {
        let n = n + 1;
        // an MCP server's .jsonl input is a session: played in lockstep, so the answer does not depend on timing
        let session = mcp || (name.ends_with(".jsonl") && serves_mcp);
        let head = format!("\"case\":{n},\"input\":\"{}\"", esc(name));
        ins.push(format!("{{\"input\":\"{}\",\"sha256\":\"{}\"}}", esc(name), hex(&sha256(data))));
        let py = || {
            let mut c = crate::proc::program(&python);
            c.arg(&file).args(&prog_args);
            run(c, data, limit, session)
        };
        let fail = |what: &str, fix: &str| {
            println!("{{{head},\"same\":false,{what},\"fix\":\"{}\"}}", esc(fix));
        };
        let p = match py() {
            Ok(r) => r,
            Err(m) => usage(&m),
        };
        let b = {
            let mut c = crate::proc::script(&bin, &file);
            c.args(&prog_args);
            match run(c, data, limit, session) {
                Ok(r) => r,
                Err(m) => usage(&m),
            }
        };
        if p.timed_out || b.timed_out {
            timeouts += 1;
            let who = if p.timed_out { "CPython" } else { "the native build" };
            fail(&format!("\"on\":\"timeout\",\"msg\":\"{who} was stopped after {} s\"", limit.as_secs()), "pass --timeout SECS for a longer run");
            continue;
        }
        if !once {
            let p2 = match py() {
                Ok(r) => r,
                Err(m) => usage(&m),
            };
            if p2.out != p.out || p2.code != p.code || norm_err(&p2.err) != norm_err(&p.err) {
                unstable += 1;
                let (line, a, b2) = first_diff(&p.out, &p2.out);
                let msg = format!("CPython itself answered differently on two runs (line {line}: '{a}' then '{b2}'), so no answer can be certified");
                fail(
                    &format!("\"on\":\"unstable\",\"msg\":\"{}\"", esc(&msg)),
                    "seed random, sort sets before printing them, leave out times and ids; or pass --once to compare one run",
                );
                continue;
            }
        }
        py_ms += p.ms;
        bv_ms += b.ms;
        let timing = format!("\"cpython_ms\":{},\"brevi_ms\":{}", p.ms, b.ms);
        if p.out != b.out {
            differ += 1;
            let (line, a, bb) = first_diff(&p.out, &b.out);
            fail(&format!("\"on\":\"stdout\",\"line\":{line},\"cpython\":\"{}\",\"brevi\":\"{}\",{timing}", esc(&a), esc(&bb)), "a wrong answer from Brevi: please report it at https://github.com/ets-public/brevi/issues; until it is fixed, run this file with python3");
        } else if p.code != b.code {
            differ += 1;
            let c = |x: Option<i32>| x.map_or("none".to_string(), |v| v.to_string());
            let last = |e: &[u8]| norm_err(e).into_iter().rev().find(|l| !l.is_empty()).unwrap_or_default();
            fail(
                &format!("\"on\":\"exit code\",\"cpython\":{},\"brevi\":{},\"cpython_err\":\"{}\",\"brevi_err\":\"{}\",{timing}", c(p.code), c(b.code), esc(&last(&p.err)), esc(&last(&b.err))),
                "a wrong answer from Brevi: please report it at https://github.com/ets-public/brevi/issues",
            );
        } else if norm_err(&p.err) != norm_err(&b.err) {
            differ += 1;
            let (pe, be) = (norm_err(&p.err).join("\n") + "\n", norm_err(&b.err).join("\n") + "\n");
            let (line, a, bb) = first_diff(pe.as_bytes(), be.as_bytes());
            fail(&format!("\"on\":\"stderr\",\"line\":{line},\"cpython\":\"{}\",\"brevi\":\"{}\",{timing}", esc(&a), esc(&bb)), "stdout and the exit code match; only stderr differs");
        } else {
            println!("{{{head},\"same\":true,{timing}}}");
        }
    }
    let _ = std::fs::remove_file(&bin);
    let bad = differ + unstable + timeouts;
    let speed = if bad == 0 && bv_ms > 0 { format!(",\"speedup\":{:.1}", py_ms as f64 / bv_ms as f64) } else { String::new() };
    // A wrong answer outranks the rest: unstable and timed out inputs certify nothing either way.
    let verdict = if differ > 0 { "differs" } else if unstable > 0 { "unstable" } else if timeouts > 0 { "timeout" } else { "identical" };
    let checked = if once { "stdout, exit code and stderr, one CPython run" } else { "stdout, exit code and stderr, CPython run twice" };
    println!(
        "{{\"verify\":\"{verdict}\",{common},\"path\":\"{}\",\"inputs\":[{}],\"args\":[{}],\"compared\":\"{checked}\",\"differ\":{differ},\"unstable\":{unstable},\"timeout\":{timeouts},\"cpython_ms\":{py_ms},\"brevi_ms\":{bv_ms}{speed}}}",
        esc(&path),
        ins.join(","),
        prog_args.iter().map(|a| format!("\"{}\"", esc(a))).collect::<Vec<_>>().join(",")
    );
    exit(if bad == 0 { 0 } else { 1 });
}
