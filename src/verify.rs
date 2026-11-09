// `brevi verify FILE.py`: proof that the native build prints what CPython prints.
//
// Builds the file natively (strict, as `brevi build --native`), then runs CPython and the built program on
// the same inputs and compares stdout, the exit code and stderr (CPython's traceback frames left out: a built
// program names the exception, not the frames). CPython runs each input twice, so output that changes from
// run to run (set order, time, unseeded random) is reported as unstable rather than certified.
//
//   brevi verify FILE.py [--input PATH]... [--once] [--mcp] [--timeout SECS] [--limit SPEC] [--release] [--no-cache] [-- ARGS...]
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
// its two runs), timeout, limit (a --limit stopped CPython or the build on an input, so it certifies nothing),
// or not built (Brevi refused the file natively, so nothing was compared). --limit holds both runs to the same
// limits (proc.rs).
// Exit 0 when every input matched, 1 when one differed, was unstable or timed out, or the file did not build,
// 2 for a usage error. --sandbox is refused: verify runs the program under CPython, which cannot hold it.
use crate::diag::esc;
use crate::dynpy::tok::{hex, sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{exit, Stdio};
use std::time::{Duration, Instant};

const USAGE: &str = "usage: brevi verify FILE.py [--input PATH]... [--once] [--mcp] [--timeout SECS] [--limit cpu=S,mem=MB,time=S,output=KB,file=MB] [--release] [--no-cache] [-- ARGS...]";

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
    // The --limit that stopped it (cpu, mem, time, output, file), if one did.
    hit: Option<&'static str>,
}

fn run(c: std::process::Command, stdin: &[u8], limit: Duration, session: bool) -> Result<Run, String> {
    run_until(c, stdin, limit, session, None)
}

// run, also stopped (as at the time limit) once `cancel` is set: `brevi run --verify` stops CPython when it
// has taken too long next to the native run.
fn run_until(mut c: std::process::Command, stdin: &[u8], limit: Duration, session: bool, cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>) -> Result<Run, String> {
    let lim = crate::proc::limits();
    crate::proc::apply(&mut c, &lim);
    let limit = lim.time.map_or(limit, |t| t.min(limit));
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
            Ok(None) if start.elapsed() > limit || cancel.as_ref().is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed)) => {
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
    let (out, err) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
    let tail = String::from_utf8_lossy(&err[err.len().saturating_sub(4096)..]).to_string();
    let hit = if timed_out && lim.time.is_some_and(|t| t <= limit) {
        Some("time")
    } else if lim.output.is_some_and(|o| (out.len() + err.len()) as u64 > o) {
        Some("output")
    } else {
        status.and_then(|s| crate::proc::limit_hit(&lim, s, &tail))
    };
    let timed_out = timed_out && hit.is_none();
    Ok(Run { out, err, code: status.and_then(|s| exit_code(s)), ms, timed_out, hit })
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
    let mut lims = crate::proc::Limits::default();
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
            "--limit" => {
                let v = value(i);
                lims.parse(&v).unwrap_or_else(|m| usage(&m));
                i += 1;
            }
            _ if a.starts_with("--limit=") => {
                lims.parse(&a["--limit=".len()..]).unwrap_or_else(|m| usage(&m));
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
    if lims.files.is_some() || lims.net.is_some() {
        usage("--limit files and net are counted by Brevi's own code under --sandbox, and CPython is not; verify does not take them");
    }
    let _ = crate::proc::LIMITS.set(lims);
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

    let (mut differ, mut unstable, mut timeouts, mut limited) = (0, 0, 0, 0);
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
        if let Some(on) = p.hit.or(b.hit) {
            limited += 1;
            let who = if p.hit.is_some() { "CPython" } else { "the native build" };
            let l = crate::proc::limits();
            let line = crate::proc::limit_line(&l, on);
            let msg = crate::json::parse(&line).and_then(|j| j.get("msg").and_then(|m| m.str()).map(String::from)).unwrap_or_default();
            fail(&format!("\"on\":\"limit\",\"limit\":\"{on}\",\"msg\":\"{who}: {}\"", esc(&msg)), &format!("raise it with --limit {on}=..., or leave the limit out"));
            continue;
        }
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
    let bad = differ + unstable + timeouts + limited;
    let speed = if bad == 0 && bv_ms > 0 { format!(",\"speedup\":{:.1}", py_ms as f64 / bv_ms as f64) } else { String::new() };
    // A wrong answer outranks the rest: unstable and timed out inputs certify nothing either way.
    let verdict = if differ > 0 { "differs" } else if unstable > 0 { "unstable" } else if timeouts > 0 { "timeout" } else if limited > 0 { "limit" } else { "identical" };
    let checked = if once { "stdout, exit code and stderr, one CPython run" } else { "stdout, exit code and stderr, CPython run twice" };
    println!(
        "{{\"verify\":\"{verdict}\",{common},\"path\":\"{}\",\"inputs\":[{}],\"args\":[{}],\"compared\":\"{checked}\",\"differ\":{differ},\"unstable\":{unstable},\"timeout\":{timeouts},\"limit\":{limited},\"cpython_ms\":{py_ms},\"brevi_ms\":{bv_ms}{speed}}}",
        esc(&path),
        ins.join(","),
        prog_args.iter().map(|a| format!("\"{}\"", esc(a))).collect::<Vec<_>>().join(",")
    );
    exit(if bad == 0 { 0 } else { 1 });
}

// ---------------------------------------------------------------------------------------------------------
// `brevi run --verify FILE.py [--strict] [--timeout SECS] [--limit SPEC] [--release] [--no-cache] [-- ARGS]`
// (docs/VERIFY_BY_DEFAULT.md): runs the program once natively and checks that run against CPython on the
// same stdin and args, CPython running alongside the binary.
//   same       the native output, its exit code; the certificate on stderr
//   different  CPython's output and exit code (the answer the program means), and on stderr the first
//              differing line; logged for `brevi report`. --strict prints neither run's output and exits 1.
//   refused    a program a second run would change (it writes files, uses the network or native code, reads
//              the clock, unseeded random numbers or the environment): a plain `brevi run`, unverified
//   not built  a plain `brevi run`: CPython runs it, so there is nothing to compare
//   timeout    CPython took over 10x the native time (at least 2 s): the native answer, marked unverified
//              (--strict waits for CPython)
// The verdict is one JSON line on stderr, {"verify":...}, after the program's own stderr.

fn run_usage(msg: &str) -> ! {
    eprintln!("{{\"verify\":\"error\",\"msg\":\"{}\",\"fix\":\"usage: brevi run --verify FILE.py [--strict] [--timeout SECS] [--limit SPEC] [--release] [--no-cache] [-- ARGS...]\"}}", esc(msg));
    exit(2);
}

// CPython's version, or None when there is no CPython to compare with.
fn python_version_opt(python: &str) -> Option<String> {
    let o = crate::proc::program(python).arg("-c").arg("import sys; print('%d.%d.%d' % sys.version_info[:3])").output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

// The strict native build `brevi verify` uses (shared with it through the result cache).
fn build_native(file: &str, release: bool) -> (crate::runcache::Done, PathBuf) {
    let stem = Path::new(file).file_stem().unwrap().to_string_lossy().to_string();
    let bin = crate::build_dir_path(file).join(format!("{stem}-verify"));
    let mut args: Vec<String> = vec!["build".into(), file.to_string(), "-o".into(), bin.to_string_lossy().to_string()];
    if release {
        args.push("--release".into());
    }
    let envs = [("BREVI_NATIVE", "1"), ("BREVI_PATH", "1")];
    let built = if crate::runcache::on() {
        crate::runcache::phase(&args, file, Some(&bin), &envs, true)
    } else {
        let me = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("brevi"));
        let mut c = std::process::Command::new(me);
        c.args(&args).envs(envs).env("BREVI_NO_CACHE", "1");
        match c.output() {
            Ok(o) => crate::runcache::Done { stdout: o.stdout, stderr: o.stderr, code: o.status.code(), hit: false },
            Err(e) => crate::runcache::Done { stdout: vec![], stderr: format!("cannot start brevi build: {e}\n").into_bytes(), code: Some(1), hit: false },
        }
    };
    (built, bin)
}

// The local modules a program imports (util.py for `import util`), with the program itself first.
fn program_files(file: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    let Ok(src) = std::fs::read_to_string(file) else { return out };
    let dir = Path::new(file).parent().unwrap_or(Path::new("."));
    let mut seen = std::collections::BTreeSet::new();
    let mut todo = vec![(file.to_string(), src)];
    while let Some((name, src)) = todo.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        for l in src.lines() {
            let t = l.trim_start();
            let m = t.strip_prefix("import ").or_else(|| t.strip_prefix("from ")).unwrap_or("");
            for part in m.split(|c: char| c == ',' || c == ' ').filter(|p| !p.is_empty()).take(if t.starts_with("from ") { 1 } else { usize::MAX }) {
                let top = part.split('.').next().unwrap_or("");
                if top == "as" || top == "import" {
                    break;
                }
                let f = dir.join(format!("{top}.py"));
                if f.is_file() {
                    if let Ok(s) = std::fs::read_to_string(&f) {
                        todo.push((f.to_string_lossy().to_string(), s));
                    }
                }
            }
        }
        out.push((name, src));
    }
    out
}

// Whether `name(` is called as a whole name (not `myhash(` or `x.id(`).
fn calls(l: &str, name: &str) -> bool {
    let pat = format!("{name}(");
    l.match_indices(&pat).any(|(i, _)| i == 0 || !l[..i].chars().last().is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.'))
}

// Why a second run of the program could answer differently from the first, read from its source: the clock,
// unseeded random numbers, the environment, object ids and string hashes. None when nothing is seen.
fn unrepeatable(files: &[(String, String)]) -> Option<&'static str> {
    let all: String = files.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>().join("\n");
    let code: Vec<&str> = all.lines().map(|l| l.trim()).filter(|l| !l.starts_with('#')).collect();
    let any = |pats: &[&str]| code.iter().any(|l| pats.iter().any(|p| l.contains(p)));
    if any(&["time.time(", "time.time_ns(", "perf_counter", "monotonic(", "process_time(", ".now(", ".utcnow(", ".today(", "time.localtime(", "time.gmtime(", "time.ctime(", "time.strftime("])
        || code.iter().any(|l| l.starts_with("from time import") || l.starts_with("from datetime import"))
    {
        return Some("reads the clock");
    }
    let random = code.iter().any(|l| l.contains("random.") || l.starts_with("from random import") || l.starts_with("import random") || l.contains("np.random") || l.contains("numpy.random"));
    if random && !code.iter().any(|l| l.contains("seed(")) {
        return Some("uses random numbers without a seed");
    }
    if any(&["uuid1(", "uuid4(", "os.urandom(", "secrets.", "os.getpid(", "getppid("]) {
        return Some("uses random ids");
    }
    if any(&["os.environ", "getenv(", "sys.argv[0]", "platform."]) {
        return Some("reads the environment");
    }
    if code.iter().any(|l| calls(l, "id") || calls(l, "hash")) {
        return Some("prints object ids or string hashes");
    }
    None
}

// Why a second run would have effects a user sees twice: what the sandbox's check (nothing granted but read)
// reports for the program, as `brevi check --sandbox --fast` would.
fn side_effects(file: &str) -> Option<&'static str> {
    let me = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("brevi"));
    let o = std::process::Command::new(me).args(["check", file, "--sandbox", "--fast", "--allow", "read"]).env("BREVI_PATH", "0").env("BREVI_NO_CACHE", "1").output().ok()?;
    let out = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);
    if out.contains("needs allow write") {
        Some("writes files")
    } else if out.contains("needs allow net") {
        Some("uses the network")
    } else if out.contains("needs allow native") {
        Some("runs native code or a Python library")
    } else {
        None
    }
}

// A plain `brevi run` of the file (stdin not yet read, so the program gets it as is). Never returns.
fn plain_run(file: &str, flags: &[String], prog_args: &[String]) -> ! {
    let me = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("brevi"));
    let mut c = std::process::Command::new(me);
    c.arg("run").arg(file).args(flags).arg("--").args(prog_args);
    match c.status() {
        Ok(s) => exit(exit_code(s).unwrap_or(1)),
        Err(e) => {
            eprintln!("{{\"e\":\"io\",\"msg\":\"cannot start brevi run: {}\"}}", esc(&e.to_string()));
            exit(1)
        }
    }
}

fn opt_code(c: Option<i32>) -> String {
    c.map_or("none".to_string(), |v| v.to_string())
}

// How a native run differs from CPython's: (on, first differing line, CPython's line, Brevi's line), or None.
fn difference(p: &Run, b: &Run) -> Option<(&'static str, usize, String, String)> {
    if p.out != b.out {
        let (line, a, bb) = first_diff(&p.out, &b.out);
        return Some(("stdout", line, a, bb));
    }
    if p.code != b.code {
        return Some(("exit code", 0, opt_code(p.code), opt_code(b.code)));
    }
    let (pe, be) = (norm_err(&p.err), norm_err(&b.err));
    if pe != be {
        let (pe, be) = (pe.join("\n") + "\n", be.join("\n") + "\n");
        let (line, a, bb) = first_diff(pe.as_bytes(), be.as_bytes());
        return Some(("stderr", line, a, bb));
    }
    None
}

// Keeps a wrong answer for `brevi report`: $BREVI_CACHE/differs/TIME-SHA/ holds the program, its input and
// arguments, both runs' output, and the verdict line. Local only: nothing is sent anywhere.
fn log_differs(files: &[(String, String)], stdin: &[u8], args: &[String], p: &Run, b: &Run, line: &str) -> Option<PathBuf> {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_micros());
    let sum = hex(&sha256(files.first().map_or("", |(_, s)| s.as_str()).as_bytes()));
    let dir = crate::cache_dir().join("differs").join(format!("{secs}-{}", &sum[..12]));
    std::fs::create_dir_all(&dir).ok()?;
    for (i, (name, src)) in files.iter().enumerate() {
        let base = Path::new(name).file_name().map_or("main.py".to_string(), |n| n.to_string_lossy().to_string());
        // the program first, as main.py's own name; modules beside it
        let _ = std::fs::write(dir.join(if i == 0 { format!("program-{base}") } else { base }), src);
    }
    let _ = std::fs::write(dir.join("stdin"), stdin);
    let _ = std::fs::write(dir.join("args.json"), format!("[{}]", args.iter().map(|a| format!("\"{}\"", esc(a))).collect::<Vec<_>>().join(",")));
    let _ = std::fs::write(dir.join("cpython.out"), &p.out);
    let _ = std::fs::write(dir.join("cpython.err"), &p.err);
    let _ = std::fs::write(dir.join("brevi.out"), &b.out);
    let _ = std::fs::write(dir.join("brevi.err"), &b.err);
    let _ = std::fs::write(dir.join("verdict.json"), format!("{line}\n"));
    Some(dir)
}

pub fn run_main(argv: &[String]) -> ! {
    let dash = argv.iter().position(|a| a == "--").unwrap_or(argv.len());
    let prog_args: Vec<String> = argv.get(dash + 1..).unwrap_or(&[]).to_vec();
    let (mut file, mut strict, mut release) = (None, false, false);
    let mut limit = Duration::from_secs(60);
    let mut lims = crate::proc::Limits::default();
    // passed on to a plain `brevi run` when nothing is verified
    let mut pass: Vec<String> = vec![];
    let mut i = 0;
    while i < dash {
        let a = argv[i].as_str();
        let value = |i: usize| argv.get(i + 1).filter(|_| i + 1 < dash).cloned().unwrap_or_else(|| run_usage(&format!("{a} needs a value")));
        match a {
            "--strict" => strict = true,
            "--release" => {
                release = true;
                pass.push(a.into());
            }
            "--traceback" | "--explain" => pass.push(a.into()),
            "--no-cache" => {
                crate::runcache::turn_off();
                pass.push(a.into());
            }
            "--timeout" => {
                let v = value(i);
                limit = Duration::from_secs(v.parse::<u64>().ok().filter(|s| *s > 0).unwrap_or_else(|| run_usage("--timeout takes whole seconds above 0")));
                i += 1;
            }
            "--limit" => {
                let v = value(i);
                lims.parse(&v).unwrap_or_else(|m| run_usage(&m));
                pass.extend([a.to_string(), v]);
                i += 1;
            }
            _ if a.starts_with("--limit=") => {
                lims.parse(&a["--limit=".len()..]).unwrap_or_else(|m| run_usage(&m));
                pass.push(a.into());
            }
            _ if a == "--sandbox" || a.starts_with("--allow") => run_usage(
                "--verify runs the program under CPython too, which the sandbox cannot hold to its grants; use --verify only for code you would run with python3",
            ),
            _ if a.starts_with('-') => run_usage(&format!("run --verify has no option {a}")),
            _ if file.is_none() => file = Some(a.to_string()),
            _ => run_usage(&format!("one file at a time ({a} is a second one); pass program arguments after --")),
        }
        i += 1;
    }
    if lims.files.is_some() || lims.net.is_some() {
        run_usage("--limit files and net are counted by Brevi's own code under --sandbox, and CPython is not; --verify does not take them");
    }
    let _ = crate::proc::LIMITS.set(lims);
    let Some(file) = file else { run_usage("run --verify needs a .py file") };
    if !file.ends_with(".py") {
        run_usage("--verify compares a .py file with CPython; a .bv file has no CPython answer to compare with (use brevi test)");
    }
    let src = std::fs::read(&file).unwrap_or_else(|e| run_usage(&format!("cannot read {file}: {e}")));
    let python = std::env::var("BREVI_PYTHON").unwrap_or_else(|_| "python3".into());
    let pyver = {
        let python = python.clone();
        std::thread::spawn(move || python_version_opt(&python))
    };
    let refuse = |why: &str| -> ! {
        eprintln!("{{\"verify\":\"refused\",\"why\":\"{}\",\"msg\":\"not checked against CPython: a second run could answer differently or repeat what it does; run as plain brevi run\"}}", esc(why));
        if strict {
            exit(1);
        }
        plain_run(&file, &pass, &prog_args)
    };
    let files = program_files(&file);
    if let Some(why) = unrepeatable(&files) {
        refuse(why);
    }
    let (built, bin) = build_native(&file, release);
    if !built.ok() {
        // Brevi refused the file natively: `brevi run` hands it to CPython, and there is nothing to compare.
        if strict {
            std::io::stdout().write_all(&built.stdout).ok();
            std::io::stderr().write_all(&built.stderr).ok();
            eprintln!("{{\"verify\":\"not built\",\"msg\":\"Brevi could not build this file natively, so nothing was compared\"}}");
            exit(1);
        }
        plain_run(&file, &pass, &prog_args);
    }
    if let Some(why) = side_effects(&file) {
        refuse(why);
    }
    let Some(pyver) = pyver.join().ok().flatten() else {
        refuse(&format!("{python} not found"));
    };
    // The build's notes, and its path line as `brevi run` shows it (typed only with BREVI_PATH=1).
    let show_path = std::env::var("BREVI_PATH").is_ok_and(|v| v == "1");
    let err = String::from_utf8_lossy(&built.stderr).to_string();
    let path_line = err.lines().find(|l| l.starts_with("{\"path\":")).map(String::from);
    let path = path_line.as_deref().and_then(crate::json::parse).and_then(|j| j.get("path").and_then(|v| v.str()).map(String::from)).unwrap_or_else(|| "typed".into());
    for l in err.lines() {
        if !l.starts_with("{\"path\":") || show_path || path != "typed" {
            eprintln!("{l}");
        }
    }
    let mut stdin = vec![];
    let _ = std::io::stdin().read_to_end(&mut stdin);

    // CPython alongside the binary.
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (tx, rx) = std::sync::mpsc::channel();
    {
        let (python, file, prog_args, stdin, cancel) = (python.clone(), file.clone(), prog_args.clone(), stdin.clone(), cancel.clone());
        std::thread::spawn(move || {
            let py = || {
                let mut c = crate::proc::program(&python);
                c.arg(&file).args(&prog_args);
                run_until(c, &stdin, limit, false, Some(cancel.clone()))
            };
            let first = py();
            let second = if strict && first.as_ref().is_ok_and(|r| !r.timed_out && r.hit.is_none()) { Some(py()) } else { None };
            let _ = tx.send((first, second));
        });
    }
    let b = {
        let mut c = crate::proc::script(&bin, &file);
        c.args(&prog_args);
        run(c, &stdin, limit, false).unwrap_or_else(|m| {
            eprintln!("{{\"e\":\"io\",\"msg\":\"{}\"}}", esc(&m));
            exit(1)
        })
    };
    let native_out = |b: &Run, line: &str| -> ! {
        std::io::stdout().write_all(&b.out).ok();
        std::io::stdout().flush().ok();
        std::io::stderr().write_all(&b.err).ok();
        eprintln!("{line}");
        exit(b.code.unwrap_or(1))
    };
    // The native run itself was stopped (a --limit or --timeout): that is the answer, as `brevi run` gives it.
    if let Some(on) = b.hit {
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        native_out(&b, &crate::proc::limit_line(&crate::proc::limits(), on));
    }
    if b.timed_out {
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        native_out(&b, &format!("{{\"e\":\"timeout\",\"msg\":\"stopped after {} s; pass --timeout SECS for a longer run\"}}", limit.as_secs()));
    }
    let wait = Duration::from_millis((b.ms as u64 * 10).max(2000));
    let got = if strict { rx.recv().ok() } else { rx.recv_timeout(wait).ok() };
    let unverified = |why: &str| -> ! {
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        if strict {
            eprintln!("{{\"verify\":\"timeout\",\"msg\":\"{}\"}}", esc(why));
            exit(1);
        }
        native_out(&b, &format!("{{\"verify\":\"timeout\",\"brevi_ms\":{},\"msg\":\"{}; the native answer above is unverified\"}}", b.ms, esc(why)))
    };
    let Some((first, second)) = got else {
        unverified(&format!("CPython had not finished {} ms after the native run", wait.as_millis()));
    };
    let p = first.unwrap_or_else(|m| refuse(&m));
    if p.hit.is_some() || p.timed_out {
        unverified("CPython was stopped at the limit");
    }
    if let Some(Ok(p2)) = second {
        if difference(&p, &p2).is_some() {
            eprintln!("{{\"verify\":\"unstable\",\"msg\":\"CPython itself answered differently on two runs, so no answer can be certified\",\"fix\":\"seed random, sort sets before printing them, leave out times and ids\"}}");
            exit(1);
        }
    }
    let common = format!(
        "\"file\":\"{}\",\"sha256\":\"{}\",\"inputs\":[\"sha256:{}\"],\"args\":[{}],\"path\":\"{}\"",
        esc(&file),
        hex(&sha256(&src)),
        hex(&sha256(&stdin)),
        prog_args.iter().map(|a| format!("\"{}\"", esc(a))).collect::<Vec<_>>().join(","),
        esc(&path)
    );
    let versions = format!("\"brevi\":\"{}\",\"cpython\":\"{}\"", env!("CARGO_PKG_VERSION"), esc(&pyver));
    match difference(&p, &b) {
        None => {
            let speed = if b.ms > 0 { format!(",\"speedup\":{:.1}", p.ms as f64 / b.ms as f64) } else { String::new() };
            let cert = format!(
                "{{\"verify\":\"identical\",{common},\"exit\":{},\"stdout_same\":true,\"stderr_same\":true,\"brevi_ms\":{},\"cpython_ms\":{}{speed},{versions}}}",
                opt_code(b.code),
                b.ms,
                p.ms
            );
            native_out(&b, &cert)
        }
        Some((on, line, cp, bv)) => {
            let at = if line > 0 { format!(",\"line\":{line}") } else { String::new() };
            let mut verdict = format!("{{\"verify\":\"differs\",\"on\":\"{on}\"{at},\"brevi\":\"{}\",\"cpython\":\"{}\",{common},\"versions\":{{{versions}}}", esc(&bv), esc(&cp));
            let logged = log_differs(&files, &stdin, &prog_args, &p, &b, &(verdict.clone() + "}"));
            if let Some(d) = &logged {
                verdict.push_str(&format!(",\"logged\":\"{}\"", esc(&d.to_string_lossy())));
            }
            let msg = if strict {
                "a wrong answer from Brevi: nothing printed (--strict); brevi report shows it, ready to file"
            } else {
                "a wrong answer from Brevi: the output above is CPython's; brevi report shows it, ready to file"
            };
            verdict.push_str(&format!(",\"msg\":\"{msg}\"}}"));
            if strict {
                eprintln!("{verdict}");
                exit(1);
            }
            std::io::stdout().write_all(&p.out).ok();
            std::io::stdout().flush().ok();
            std::io::stderr().write_all(&p.err).ok();
            eprintln!("{verdict}");
            exit(p.code.unwrap_or(1))
        }
    }
}

// `brevi report [--clear]`: the wrong answers `brevi run --verify` logged, each with a GitHub issue body to
// paste at https://github.com/ets-public/brevi/issues/new. Local only.
pub fn report(argv: &[String]) -> ! {
    let dir = crate::cache_dir().join("differs");
    let mut runs: Vec<PathBuf> = std::fs::read_dir(&dir).map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect()).unwrap_or_default();
    runs.sort();
    if argv.iter().any(|a| a == "--clear") {
        let _ = std::fs::remove_dir_all(&dir);
        println!("cleared {} logged wrong answer{}", runs.len(), if runs.len() == 1 { "" } else { "s" });
        exit(0);
    }
    if let Some(a) = argv.iter().find(|a| *a != "--clear") {
        println!("{{\"msg\":\"report has no option {}\",\"fix\":\"usage: brevi report [--clear]\"}}", esc(a));
        exit(2);
    }
    if runs.is_empty() {
        println!("no wrong answers logged ({} is empty): brevi run --verify logs one each time the native build and CPython differ", dir.display());
        exit(0);
    }
    let read = |p: &Path| String::from_utf8_lossy(&std::fs::read(p).unwrap_or_default()).to_string();
    let cut = |t: &str, n: usize| {
        let lines: Vec<&str> = t.lines().collect();
        if lines.len() <= n {
            t.trim_end().to_string()
        } else {
            format!("{}\n... ({} more lines)", lines[..n].join("\n"), lines.len() - n)
        }
    };
    println!("{} wrong answer{} logged in {}\n", runs.len(), if runs.len() == 1 { "" } else { "s" }, dir.display());
    for (n, r) in runs.iter().enumerate() {
        let verdict = read(&r.join("verdict.json"));
        let j = crate::json::parse(verdict.trim());
        let g = |k: &str| j.as_ref().and_then(|j| j.get(k)).map(|v| v.str().map(String::from).unwrap_or_else(|| crate::mcp::to_json(v))).unwrap_or_default();
        let mut files: Vec<PathBuf> = std::fs::read_dir(r).map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect()).unwrap_or_default();
        files.sort();
        let prog = files.iter().find(|f| f.file_name().is_some_and(|x| x.to_string_lossy().starts_with("program-")));
        let mods: Vec<&PathBuf> = files.iter().filter(|f| f.extension().is_some_and(|e| e == "py") && Some(*f) != prog).collect();
        let name = prog.and_then(|p| p.file_name()).map_or("main.py".into(), |x| x.to_string_lossy().trim_start_matches("program-").to_string());
        println!("## {}. {} (on {}{}) logged in {}", n + 1, g("file"), g("on"), if g("line").is_empty() { String::new() } else { format!(" line {}", g("line")) }, r.display());
        println!("--- issue body (paste at https://github.com/ets-public/brevi/issues/new) ---");
        println!("**Title:** Wrong answer: native build differs from CPython on {}\n", g("on"));
        let ver = |k: &str| j.as_ref().and_then(|j| j.get("versions")).and_then(|v| v.get(k)).and_then(|v| v.str()).unwrap_or("?").to_string();
        println!("Brevi {} gives a different answer from CPython {} on this program (found by `brevi run --verify`).\n", ver("brevi"), ver("cpython"));
        println!("`{name}`:\n```python\n{}\n```", cut(&prog.map(|p| read(p)).unwrap_or_default(), 200));
        for m in mods {
            println!("`{}`:\n```python\n{}\n```", m.file_name().unwrap().to_string_lossy(), cut(&read(m), 200));
        }
        let stdin = read(&r.join("stdin"));
        if !stdin.is_empty() {
            println!("stdin:\n```\n{}\n```", cut(&stdin, 50));
        }
        let args = read(&r.join("args.json"));
        if args.trim() != "[]" {
            println!("args: `{}`", args.trim());
        }
        println!("\nFirst difference ({}): CPython `{}`, Brevi `{}`\n", g("on"), g("cpython"), g("brevi"));
        println!("CPython stdout:\n```\n{}\n```", cut(&read(&r.join("cpython.out")), 40));
        println!("Brevi stdout:\n```\n{}\n```", cut(&read(&r.join("brevi.out")), 40));
        let (ce, be) = (read(&r.join("cpython.err")), read(&r.join("brevi.err")));
        if !ce.trim().is_empty() || !be.trim().is_empty() {
            println!("CPython stderr:\n```\n{}\n```\nBrevi stderr:\n```\n{}\n```", cut(&ce, 20), cut(&be, 20));
        }
        println!("--- end ---\n");
    }
    println!("brevi report --clear deletes them");
    exit(0)
}
