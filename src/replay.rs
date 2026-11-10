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

// Where stdin is in it, when stdin is a regular file.
#[cfg(unix)]
fn stdin_file_at() -> Option<u64> {
    use std::io::Seek;
    use std::os::unix::io::FromRawFd;
    let mut f = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(0) });
    if !f.metadata().is_ok_and(|m| m.is_file()) {
        return None;
    }
    f.stream_position().ok()
}
#[cfg(not(unix))]
fn stdin_file_at() -> Option<u64> {
    None
}
#[cfg(unix)]
fn stdin_seek(at: u64) -> bool {
    use std::io::Seek;
    use std::os::unix::io::FromRawFd;
    let mut f = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(0) });
    f.seek(std::io::SeekFrom::Start(at)).is_ok()
}
#[cfg(not(unix))]
fn stdin_seek(_: u64) -> bool {
    false
}

// Reads this process's stdin to its end, keeping it and passing it to the program running now.
fn start_feed(feed: &Arc<Mutex<Feed>>) {
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
        || std::env::var("BREVI_DYN").is_ok_and(|v| v == "0")
        || std::env::var("BREVI_REPLAY").is_ok_and(|v| v == "0");
    if off {
        return crate::proc::run(c);
    }
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let mark = std::env::temp_dir().join(format!("brevi-i64-{}-{nanos}", std::process::id()));
    let _ = std::fs::remove_file(&mark);
    // stdout and stderr stay the program's own (no copy through this process): the prelude counts and hashes
    // what it writes, for the second run to skip.
    let merged = same_target();
    c.env("BREVI_I64_REPLAY", &mark);
    if merged {
        c.env("BREVI_I64_MERGED", "1");
    }
    // stdin too stays the program's own on Linux and macOS: the prelude reads it through its own buffer and
    // hands what it took to the second run (MARK.in). Elsewhere it is passed through here and kept.
    let direct_in = cfg!(unix);
    if !direct_in {
        c.stdin(Stdio::piped());
    }
    let start = stdin_file_at();
    let mut child = c.spawn()?;
    let feed = Arc::new(Mutex::new(Feed { to: child.stdin.take(), ..Default::default() }));
    if !direct_in {
        start_feed(&feed);
    }
    let status = child.wait()?;
    let mut inp = mark.clone().into_os_string();
    inp.push(".in");
    let taken = std::fs::read(&inp);
    let _ = std::fs::remove_file(&inp);
    let Ok(tally) = std::fs::read_to_string(&mark) else { return Ok(crate::proc::code(status)) };
    let _ = std::fs::remove_file(&mark);
    if direct_in {
        feed.lock().unwrap_or_else(|e| e.into_inner()).kept.buf = taken.unwrap_or_default();
    }
    // "what\nN_OUT H_OUT N_ERR H_ERR BAD" (BAD: false, output or input; stream 0 is both together when merged).
    let (what, nums) = tally.split_once('\n').unwrap_or((tally.as_str(), ""));
    let what = what.to_string();
    let f: Vec<&str> = nums.split_whitespace().collect();
    let num = |i: usize| f.get(i).and_then(|x| x.parse::<u64>().ok());
    let skips = match (num(0), num(1), num(2), num(3), f.get(4)) {
        (Some(a), Some(b), Some(c), Some(d), Some(&"false")) => [(a, b), (c, d)],
        (.., why) => {
            let why = if why == Some(&"input") { "it read more input than Brevi keeps for a second run (64 MB), or its input could not be read" } else { "its output could not all be written" };
            return Ok(stop(&format!("the typed build stopped: {what}, which its 64-bit numbers cannot hold exactly; Brevi cannot re-run this program on the dynamic build because {why}. Run it with BREVI_DYN=1 for Python's exact values")));
        }
    };
    // stdin a regular file: the second run reads it again from where the first started.
    let reread = direct_in && start.is_some_and(stdin_seek);
    if direct_in && start.is_some() && !reread {
        return Ok(stop(&format!("the typed build stopped: {what}, which its 64-bit numbers cannot hold exactly; Brevi cannot re-run this program on the dynamic build because it could not read its input again. Run it with BREVI_DYN=1 for Python's exact values")));
    }
    if feed.lock().unwrap_or_else(|e| e.into_inner()).kept.over {
        return Ok(stop(&format!("the typed build stopped: {what}, which its 64-bit numbers cannot hold exactly; Brevi cannot re-run this program on the dynamic build because it read more than 64 MB of input. Run it with BREVI_DYN=1 for Python's exact values")));
    }
    // The dynamic build of the same file, as BREVI_DYN=1 would make it.
    let no = |why: &str| stop(&format!("the typed build stopped: {what}, which its 64-bit numbers cannot hold exactly, and {why}. Run it with BREVI_DYN=1 for Python's exact values"));
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
    d.args(args);
    if !reread {
        d.stdin(Stdio::piped());
    }
    let one = outputs(&mut d, merged)?;
    let mut dchild = d.spawn()?;
    drop(d);
    // The same stdin: what the first run was given, then the rest as it comes.
    if !reread {
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
        if direct_in {
            start_feed(&feed);
        }
    }
    let differ = Arc::new(AtomicBool::new(false));
    let status = pass_on(&mut dchild, one, if merged { vec![skips[0]] } else { skips.to_vec() }, &differ)?;
    if differ.load(Ordering::SeqCst) {
        return Ok(stop("the dynamic build's re-run of this program (after the typed build stopped on a value it cannot hold) did not write what the typed run had already written, so it was stopped. Run it with BREVI_DYN=1"));
    }
    Ok(crate::proc::code(status))
}

// Whether this process's stdout and stderr are the same file or pipe (`2>&1`): the program then writes to both
// in one order, so the second run gets one pipe for both and the first run's two counts are one.
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

// The second run's stdout and stderr: two pipes, or one shared by both (returned).
fn outputs(c: &mut Command, merged: bool) -> std::io::Result<Option<std::io::PipeReader>> {
    if !merged {
        c.stdout(Stdio::piped()).stderr(Stdio::piped());
        return Ok(None);
    }
    let (r, w) = std::io::pipe()?;
    c.stdout(w.try_clone()?).stderr(w);
    Ok(Some(r))
}

// Passes the second run's stdout and stderr on as they come. `skips` is, per stream (one when they share a
// pipe), how many bytes the first run already wrote and their hash: the second run must write the same first
// (checked by hash), and that part is not passed on again. `differ` is set, and the run stopped, when it
// writes something else or less.
fn pass_on(child: &mut Child, one: Option<std::io::PipeReader>, skips: Vec<(u64, u64)>, differ: &Arc<AtomicBool>) -> std::io::Result<ExitStatus> {
    let pid = child.id();
    let streams: Vec<(Box<dyn Read + Send>, bool, (u64, u64))> = [
        one.map(|r| (Box::new(r) as Box<dyn Read + Send>, true, skips[0])),
        child.stdout.take().map(|r| (Box::new(r) as Box<dyn Read + Send>, true, skips[0])),
        child.stderr.take().map(|r| (Box::new(r) as Box<dyn Read + Send>, false, skips.get(1).copied().unwrap_or((0, Hs::new().end())))),
    ]
    .into_iter()
    .flatten()
    .collect();
    let mut pumps = vec![];
    // A faithful second run writes all of both prefixes before anything new on either stream: new output waits
    // until each stream's prefix has matched (or its stream ended, short, which sets differ).
    let gate = Arc::new((Mutex::new(streams.iter().filter(|s| s.2 .0 > 0).count()), std::sync::Condvar::new()));
    for (mut r, is_out, (n, want)) in streams {
        let differ = differ.clone();
        let gate = gate.clone();
        pumps.push(std::thread::spawn(move || {
            let mut open = n > 0;
            let pass = |open: &mut bool| {
                if std::mem::take(open) {
                    let mut k = gate.0.lock().unwrap_or_else(|e| e.into_inner());
                    *k -= 1;
                    gate.1.notify_all();
                }
            };
            let mut seen = 0u64;
            let mut hs = Hs::new();
            let mut buf = vec![0u8; 65536];
            loop {
                let got = match r.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(k) => k,
                };
                let mut chunk = &buf[..got];
                if seen < n {
                    let k = ((n - seen) as usize).min(chunk.len());
                    hs.add(&chunk[..k]);
                    seen += k as u64;
                    chunk = &chunk[k..];
                    if seen == n {
                        if hs.end() != want {
                            differ.store(true, Ordering::SeqCst);
                            crate::proc::stop_pid(pid);
                            break;
                        }
                        pass(&mut open);
                    }
                }
                if chunk.is_empty() {
                    continue;
                }
                {
                    let mut k = gate.0.lock().unwrap_or_else(|e| e.into_inner());
                    while *k > 0 {
                        k = gate.1.wait(k).unwrap_or_else(|e| e.into_inner());
                    }
                }
                if differ.load(Ordering::SeqCst) {
                    break;
                }
                if is_out {
                    let mut o = std::io::stdout().lock();
                    let _ = o.write_all(chunk).and_then(|_| o.flush());
                } else {
                    let _ = std::io::stderr().write_all(chunk);
                }
            }
            if seen < n {
                differ.store(true, Ordering::SeqCst);
            }
            pass(&mut open);
        }));
    }
    let status = child.wait()?;
    // The pumps end at the program's end of output; one held open by something it started is not waited for.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        pumps.into_iter().for_each(|p| {
            let _ = p.join();
        });
        let _ = tx.send(());
    });
    let _ = rx.recv_timeout(std::time::Duration::from_secs(2));
    Ok(status)
}

// The prelude's AxHs (src/prelude.rs): the same hash of the same bytes, however they were split into writes.
#[derive(Clone, Copy)]
struct Hs {
    h: u64,
    tail: u64,
    tn: u32,
}
impl Hs {
    const fn new() -> Hs {
        Hs { h: 0x243F_6A88_85A3_08D3, tail: 0, tn: 0 }
    }
    fn word(&mut self, w: u64) {
        self.h = (self.h ^ w).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.h ^= self.h >> 31;
    }
    fn add(&mut self, mut b: &[u8]) {
        while self.tn != 0 && !b.is_empty() {
            self.tail |= (b[0] as u64) << (8 * self.tn);
            self.tn = (self.tn + 1) % 8;
            b = &b[1..];
            if self.tn == 0 {
                let t = self.tail;
                self.word(t);
                self.tail = 0;
            }
        }
        let mut it = b.chunks_exact(8);
        for c in &mut it {
            self.word(u64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]));
        }
        for &x in it.remainder() {
            self.tail |= (x as u64) << (8 * self.tn);
            self.tn += 1;
        }
    }
    fn end(&self) -> u64 {
        let mut c = *self;
        c.word(c.tail ^ ((c.tn as u64) << 59));
        c.h
    }
}

#[cfg(test)]
mod tests {
    // The prelude's copy of the hash must stay this one: the second run's prefix is checked with it.
    #[test]
    fn hash_matches_prelude_and_ignores_splits() {
        let pre = include_str!("prelude.rs");
        let a = pre.find("impl AxHs {").unwrap();
        let b = include_str!("replay.rs");
        let c = b.find("impl Hs {").unwrap();
        let body = |s: &str, at: usize| s[at..at + s[at..].find("\n}\n").unwrap()].replace("AxHs", "Hs").replace("    #[inline]\n", "");
        assert_eq!(body(pre, a), body(b, c));
        let data: Vec<u8> = (0..1000u32).map(|i| (i * 7 % 251) as u8).collect();
        let mut whole = super::Hs::new();
        whole.add(&data);
        for split in [1usize, 3, 7, 8, 9, 64] {
            let mut h = super::Hs::new();
            data.chunks(split).for_each(|c| h.add(c));
            assert_eq!(h.end(), whole.end());
        }
    }
}
