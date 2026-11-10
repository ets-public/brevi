// An int past 64 bits on the typed build: run the program again on the dynamic build.
//
// Python ints have no size limit; the typed build's ints are 64 bits, so a correct program (a factorial, a
// hash, int(1e20)) can outgrow them. The typed build never wraps or caps such a value: the prelude stops the
// run at once (ax_i64_stop). `brevi run` runs a Python file's typed binary through `run` here, which passes
// its stdin, stdout and stderr through this process and keeps a copy of each. When the binary stops that way,
// the program is run again from the start on the dynamic build, whose ints are Python's: it is given the same
// stdin, and of its stdout and stderr only what the first run had not already written is passed on. What it
// writes before that must be what the first run wrote, byte for byte; if not, the run stops with an error
// rather than show output from two different runs.
//
// A second run must not do anything twice, so the build lets the binary ask for it only when the program
// touches nothing outside stdin, stdout, stderr and files it reads (why_not: caps.rs proves it writes no file,
// reaches no network and runs no native code). Anywhere it cannot be done (that program, a binary run by
// itself, --limit, more than 64 MB of input or output to hold) the run stops with an error that says why.
use std::io::{Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// The most stdin, stdout or stderr kept for a second run.
const CAP: usize = 64 << 20;

// "" when a run of this Python file can be repeated on the dynamic build, else why not (shown in the error).
pub fn why_not(file: &str) -> String {
    let Ok(src) = std::fs::read_to_string(file) else { return "its source could not be read".into() };
    let src = crate::modules::fill_file_name(&src, std::path::Path::new(file));
    let src = crate::modules::expand(file, &src).map(|x| x.0).unwrap_or(src);
    let (code, _, _) = crate::caps::split(&src);
    if crate::caps::check_src(&code, &["read".to_string()]).is_empty() {
        String::new()
    } else {
        "it writes files, reaches the network or runs native code, which a second run would do again".into()
    }
}

#[derive(Default)]
struct Kept {
    buf: Vec<u8>,
    over: bool,
}
impl Kept {
    fn keep(&mut self, b: &[u8]) {
        if self.over {
            return;
        }
        if self.buf.len() + b.len() > CAP {
            self.over = true;
            self.buf = Vec::new();
        } else {
            self.buf.extend_from_slice(b);
        }
    }
}

// This process's stdin, read by one thread for the whole run: what it reads is kept and written to the
// program running now (`to`).
#[derive(Default)]
struct Feed {
    kept: Kept,
    to: Option<std::process::ChildStdin>,
    eof: bool,
}

fn stop(msg: &str) -> i32 {
    let _ = std::io::stdout().flush();
    eprintln!("{{\"e\":\"runtime\",\"msg\":\"{}\"}}", crate::diag::esc(msg));
    1
}

// Runs a typed binary (`brevi run`), and again on the dynamic build if it stops at an int past 64 bits.
pub fn run(mut c: Command, file: &str, args: &[String], release: bool) -> std::io::Result<i32> {
    let off = !crate::is_py(file)
        || !crate::proc::limits().is_empty()
        || std::env::var_os("BREVI_I64_REPLAY").is_some()
        || std::env::var("BREVI_DYN").is_ok_and(|v| v == "0");
    if off {
        return crate::proc::run(c);
    }
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let mark = std::env::temp_dir().join(format!("brevi-i64-{}-{nanos}", std::process::id()));
    let _ = std::fs::remove_file(&mark);
    let merged = same_target();
    c.env("BREVI_I64_REPLAY", &mark).stdin(Stdio::piped());
    let one = outputs(&mut c, merged)?;
    let mut child = c.spawn()?;
    // The Command holds the write end of a shared pipe: the reader sees the end only once it is gone.
    drop(c);
    let feed = Arc::new(Mutex::new(Feed { to: child.stdin.take(), ..Default::default() }));
    {
        let feed = feed.clone();
        std::thread::spawn(move || {
            let mut inp = std::io::stdin();
            let mut buf = vec![0u8; 65536];
            loop {
                let n = inp.read(&mut buf).unwrap_or(0);
                let mut f = feed.lock().unwrap_or_else(|e| e.into_inner());
                if n == 0 {
                    f.eof = true;
                    f.to = None;
                    return;
                }
                f.kept.keep(&buf[..n]);
                if f.to.as_mut().is_some_and(|t| t.write_all(&buf[..n]).is_err()) {
                    f.to = None;
                }
            }
        });
    }
    let (status, out, err) = pass_on(&mut child, one, Vec::new(), Vec::new(), &Arc::new(AtomicBool::new(false)))?;
    let Ok(what) = std::fs::read_to_string(&mark) else { return Ok(crate::proc::code(status)) };
    let _ = std::fs::remove_file(&mark);
    if out.over || err.over || feed.lock().unwrap_or_else(|e| e.into_inner()).kept.over {
        return Ok(stop(&format!("OverflowError: the typed build stopped ({what}: its ints are 64 bits, Python's have no limit); Brevi cannot re-run this program on the dynamic build because it read or wrote more than 64 MB. Run it with BREVI_DYN=1 for exact big ints")));
    }
    // The dynamic build of the same file, as BREVI_DYN=1 would make it.
    let no = |why: &str| stop(&format!("OverflowError: the typed build stopped ({what}: its ints are 64 bits, Python's have no limit), and {why}. Run it with BREVI_DYN=1 for exact big ints"));
    let rust = match crate::dynpy::to_rust(file) {
        Ok(r) => r,
        Err((_, line, msg)) => return Ok(no(&format!("the dynamic build cannot build it (line {line}: {msg})"))),
    };
    let dir = crate::build_dir(file);
    let stem = std::path::Path::new(file).file_stem().unwrap_or_default().to_string_lossy().to_string();
    let bin = dir.join(format!("{stem}-dyn"));
    if crate::dynpy::compile(&rust, &dir, &bin, release, &crate::cache_dir(), crate::rustc_version()).is_err() {
        return Ok(no("the dynamic build's code did not compile"));
    }
    crate::say_path("dynamic", "", 0, None, &format!("the typed build stopped ({what}), so it ran again on the dynamic build"), &crate::dyn_py_field(), false);
    let mut d = crate::proc::script(&bin, file);
    d.args(args).stdin(Stdio::piped());
    let one = outputs(&mut d, merged)?;
    let mut dchild = d.spawn()?;
    drop(d);
    // The same stdin: what the first run was given, then the rest as it comes.
    {
        let mut f = feed.lock().unwrap_or_else(|e| e.into_inner());
        let mut to = dchild.stdin.take();
        let kept = std::mem::take(&mut f.kept.buf);
        // Written by another thread so a program that reads none of it does not hold this one; the feed stays
        // locked until it is, so later input comes after it.
        let feed2 = feed.clone();
        drop(f);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut f = feed2.lock().unwrap_or_else(|e| e.into_inner());
            let _ = tx.send(());
            if to.as_mut().is_some_and(|t| t.write_all(&kept).is_err()) {
                to = None;
            }
            f.kept.buf = kept;
            f.to = if f.eof { None } else { to };
        });
        let _ = rx.recv();
    }
    let differ = Arc::new(AtomicBool::new(false));
    let (status, _, _) = pass_on(&mut dchild, one, out.buf, err.buf, &differ)?;
    if differ.load(Ordering::SeqCst) {
        return Ok(stop("the dynamic build's re-run of this program (after an int passed 64 bits on the typed build) did not write what the typed run had already written, so it was stopped. Run it with BREVI_DYN=1"));
    }
    Ok(crate::proc::code(status))
}

// Passes the child's stdout and stderr on as they come and keeps a copy; `skip_out`/`skip_err` are what an
// earlier run already wrote: the child must write the same first, and it is not passed on again (`differ`
// is set, and the child stopped, when it writes something else or less).
// Whether this process's stdout and stderr are the same file or pipe (`2>&1`): the program then gets one pipe for
// both, so what it writes to each stays in the order it wrote it.
#[cfg(unix)]
fn same_target() -> bool {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::io::FromRawFd;
    let meta = |fd| {
        let f = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(fd) });
        f.metadata().ok().map(|m| (m.dev(), m.ino()))
    };
    matches!((meta(1), meta(2)), (Some(a), Some(b)) if a == b)
}
#[cfg(not(unix))]
fn same_target() -> bool {
    false
}

// The program's stdout and stderr: two pipes, or one shared by both (returned).
fn outputs(c: &mut Command, merged: bool) -> std::io::Result<Option<std::io::PipeReader>> {
    if !merged {
        c.stdout(Stdio::piped()).stderr(Stdio::piped());
        return Ok(None);
    }
    let (r, w) = std::io::pipe()?;
    c.stdout(w.try_clone()?).stderr(w);
    Ok(Some(r))
}

fn pass_on(child: &mut Child, one: Option<std::io::PipeReader>, skip_out: Vec<u8>, skip_err: Vec<u8>, differ: &Arc<AtomicBool>) -> std::io::Result<(ExitStatus, Kept, Kept)> {
    let pid = child.id();
    let streams: Vec<(Box<dyn Read + Send>, bool, Vec<u8>)> = [
        one.map(|r| (Box::new(r) as Box<dyn Read + Send>, true, skip_out.clone())),
        child.stdout.take().map(|r| (Box::new(r) as Box<dyn Read + Send>, true, skip_out)),
        child.stderr.take().map(|r| (Box::new(r) as Box<dyn Read + Send>, false, skip_err)),
    ]
    .into_iter()
    .flatten()
    .collect();
    let mut pumps = vec![];
    // A faithful second run writes all of both prefixes before anything new on either stream: new output waits
    // until each stream's prefix has matched (or its stream ended, short, which sets differ).
    let gate = Arc::new((Mutex::new(streams.iter().filter(|s| !s.2.is_empty()).count()), std::sync::Condvar::new()));
    for (mut r, is_out, skip) in streams {
        let differ = differ.clone();
        let gate = gate.clone();
        pumps.push(std::thread::spawn(move || {
            let mut open = !skip.is_empty();
            let pass = |open: &mut bool| {
                if std::mem::take(open) {
                    let mut n = gate.0.lock().unwrap_or_else(|e| e.into_inner());
                    *n -= 1;
                    gate.1.notify_all();
                }
            };
            let mut kept = Kept::default();
            let mut at = 0usize;
            let mut buf = vec![0u8; 65536];
            loop {
                let n = match r.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                let mut chunk = &buf[..n];
                if at < skip.len() {
                    let k = (skip.len() - at).min(chunk.len());
                    if chunk[..k] != skip[at..at + k] {
                        differ.store(true, Ordering::SeqCst);
                        crate::proc::stop_pid(pid);
                        break;
                    }
                    at += k;
                    chunk = &chunk[k..];
                    if at == skip.len() {
                        pass(&mut open);
                    }
                }
                // A program that prints line by line would otherwise be woken for each line: a short read
                // waits a moment, so the next one takes what it wrote meanwhile (the pipe holds it).
                if n < 4096 {
                    std::thread::sleep(std::time::Duration::from_micros(500));
                }
                if chunk.is_empty() {
                    continue;
                }
                {
                    let mut n = gate.0.lock().unwrap_or_else(|e| e.into_inner());
                    while *n > 0 {
                        n = gate.1.wait(n).unwrap_or_else(|e| e.into_inner());
                    }
                }
                if differ.load(Ordering::SeqCst) {
                    break;
                }
                kept.keep(chunk);
                if is_out {
                    let mut o = std::io::stdout().lock();
                    let _ = o.write_all(chunk).and_then(|_| o.flush());
                } else {
                    let _ = std::io::stderr().write_all(chunk);
                }
            }
            if at < skip.len() {
                differ.store(true, Ordering::SeqCst);
            }
            pass(&mut open);
            kept
        }));
    }
    let status = child.wait()?;
    // The pumps end at the program's end of output; one held open by something it started is not waited for.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let ks: Vec<Kept> = pumps.into_iter().map(|p| p.join().unwrap_or_default()).collect();
        let _ = tx.send(ks);
    });
    // Output not all read then cannot be compared with a second run's.
    let held = || Kept { buf: Vec::new(), over: true };
    let mut ks = rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap_or_else(|_| vec![held(), held()]).into_iter();
    let (out, err) = (ks.next().unwrap_or_default(), ks.next().unwrap_or_default());
    Ok((status, out, err))
}
