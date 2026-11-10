// Starting the programs Brevi builds, and stopping whole process trees (used by `brevi run` and `brevi mcp`).
use std::process::{Child, Command, ExitStatus};

#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn setrlimit(resource: i32, rlim: *const [u64; 2]) -> i32;
    fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
}

// Linux: the child is killed when the process that started it dies, so a program cannot outlive `brevi run`
// or the MCP server (even when they are killed without warning).
pub fn die_with_parent(c: &mut Command) {
    #[cfg(target_os = "linux")]
    unsafe {
        use std::os::unix::process::CommandExt;
        c.pre_exec(|| {
            prctl(1, 9, 0, 0, 0); // PR_SET_PDEATHSIG, SIGKILL
            Ok(())
        });
    }
    #[cfg(not(target_os = "linux"))]
    let _ = c;
}

// A command for a built program (or the python3 fallback). With BREVI_MEM_MB set (Linux), the program gets at
// most that much address space, so a runaway allocation stops with an error instead of taking the machine.
// `brevi run x.py`: the program sees argv[0] = "x.py", as under `python3 x.py` (argparse's usage line names it).
pub fn script(bin: impl AsRef<std::ffi::OsStr>, file: &str) -> Command {
    #[allow(unused_mut)]
    let mut c = program(bin);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.arg0(file);
    }
    c
}

pub fn program(bin: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut c = Command::new(bin);
    die_with_parent(&mut c);
    apply(&mut c, &Limits::default());
    c
}

// ---- Resource limits (`--limit cpu=2,mem=256,time=5,output=64,file=10`) ----
//
// The caller's limits on one program run, beside the sandbox's capability grants (caps.rs): CPU seconds,
// address space (MB), wall-clock time (seconds), bytes written to stdout and stderr together (KB) and the size
// of any one file the program writes (MB). cpu, mem and file are kernel limits (setrlimit) set in the child
// before it starts, so the program cannot raise them; time and output are held by the process that started it,
// which stops the program when either runs out. They hold the program, not the build.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Limits {
    pub cpu: Option<u64>,
    pub mem: Option<u64>,
    pub time: Option<std::time::Duration>,
    pub output: Option<u64>,
    pub file: Option<u64>,
    // Threads at once (Linux: the process's thread count, checked as it runs).
    pub threads: Option<u64>,
    // Open file descriptors at once (RLIMIT_NOFILE: an open past it fails with "Too many open files").
    pub fds: Option<u64>,
    // MB written other than to stdout and stderr: files, pipes, sockets (Linux: the kernel's count of bytes
    // the process wrote, /proc/PID/io wchar, checked as it runs).
    pub written: Option<u64>,
    // Files the program creates, and bytes it sends and receives over the network, counted by Brevi's own file
    // and network layers in the program (fsscope.rs); so only under --sandbox without CPython or native code.
    pub files: Option<u64>,
    pub net: Option<u64>,
}

pub static LIMITS: std::sync::OnceLock<Limits> = std::sync::OnceLock::new();

pub fn limits() -> Limits {
    LIMITS.get().copied().unwrap_or_default()
}

// The exit code of a program stopped by one of its limits (as timeout(1) uses for its time limit).
pub const LIMIT_EXIT: i32 = 124;

impl Limits {
    pub fn is_empty(&self) -> bool {
        *self == Limits::default()
    }

    // Parses `cpu=2,mem=256mb,time=1.5,output=64kb,file=10` into these limits (later names win). Plain numbers
    // are seconds for cpu and time, MB for mem and file, and KB for output; sizes also take kb, mb or gb and
    // times s or ms. Refuses a limit this OS cannot enforce rather than run the program without it.
    pub fn parse(&mut self, spec: &str) -> Result<(), String> {
        const HINT: &str = "--limit takes cpu=SECS, mem=MB, time=SECS, output=KB, file=MB, written=MB, net=MB, threads=N, fds=N, files=N, comma separated (cpu=2,mem=256,time=5)";
        for part in spec.split([',', ' ']).map(str::trim).filter(|p| !p.is_empty()) {
            let Some((k, v)) = part.split_once('=') else { return Err(format!("{HINT}; '{part}' has no value")) };
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim().to_ascii_lowercase());
            let bad = || format!("{HINT}; '{part}' is not a number above 0");
            match k.as_str() {
                "cpu" | "time" => {
                    let (num, scale) = match v.strip_suffix("ms") {
                        Some(n) => (n, 0.001),
                        None => (v.strip_suffix('s').unwrap_or(&v), 1.0),
                    };
                    let secs = num.trim().parse::<f64>().ok().map(|n| n * scale).filter(|n| n.is_finite() && *n > 0.0 && *n <= 86_400.0).ok_or_else(bad)?;
                    if k == "cpu" {
                        // The kernel counts whole seconds.
                        self.cpu = Some(secs.ceil() as u64);
                    } else {
                        self.time = Some(std::time::Duration::from_secs_f64(secs));
                    }
                }
                "threads" | "fds" | "files" => {
                    let n = v.parse::<u64>().ok().filter(|n| *n > 0 && *n <= 1 << 40).ok_or_else(bad)?;
                    match k.as_str() {
                        "threads" => self.threads = Some(n),
                        "fds" => self.fds = Some(n.max(4)),
                        _ => self.files = Some(n),
                    }
                }
                "mem" | "output" | "file" | "written" | "net" => {
                    let unit = if k == "output" { 1u64 << 10 } else { 1 << 20 };
                    let (num, scale) = [("gb", 1u64 << 30), ("g", 1 << 30), ("mb", 1 << 20), ("m", 1 << 20), ("kb", 1 << 10), ("k", 1 << 10), ("b", 1)]
                        .iter()
                        .find_map(|(s, m)| v.strip_suffix(s).map(|n| (n, *m)))
                        .unwrap_or((&v, unit));
                    let bytes = num.trim().parse::<f64>().ok().filter(|n| n.is_finite() && *n > 0.0).map(|n| (n * scale as f64) as u64).filter(|b| *b > 0).ok_or_else(bad)?;
                    match k.as_str() {
                        // mem and file are kept in MB, at least 1.
                        "mem" => self.mem = Some(bytes.div_ceil(1 << 20)),
                        "file" => self.file = Some(bytes.div_ceil(1 << 20)),
                        "written" => self.written = Some(bytes),
                        "net" => self.net = Some(bytes),
                        _ => self.output = Some(bytes),
                    }
                }
                _ => return Err(format!("{HINT}; there is no limit '{k}'")),
            }
        }
        if cfg!(windows) && self.fds.is_some() {
            return Err("--limit fds is not enforced on Windows yet; leave it out, or run on Linux or macOS".into());
        }
        if !cfg!(target_os = "linux") && (self.threads.is_some() || self.written.is_some()) {
            return Err("--limit threads and written are enforced on Linux only (they read the kernel's counts in /proc); leave them out, or run on Linux".into());
        }
        if cfg!(target_os = "macos") && self.mem.is_some() {
            return Err("--limit mem is not enforced on macOS (the kernel ignores an address-space limit); leave it out, or run on Linux".into());
        }
        Ok(())
    }

    // The limits as one flag value, for the brevi processes this one starts: cpu=2,mem=256,...
    pub fn spec(&self) -> String {
        let mut p = vec![];
        if let Some(c) = self.cpu {
            p.push(format!("cpu={c}"));
        }
        if let Some(m) = self.mem {
            p.push(format!("mem={m}"));
        }
        if let Some(t) = self.time {
            p.push(format!("time={}ms", t.as_millis().max(1)));
        }
        if let Some(o) = self.output {
            p.push(format!("output={o}b"));
        }
        if let Some(f) = self.file {
            p.push(format!("file={f}"));
        }
        if let Some(n) = self.threads {
            p.push(format!("threads={n}"));
        }
        if let Some(n) = self.fds {
            p.push(format!("fds={n}"));
        }
        if let Some(b) = self.written {
            p.push(format!("written={b}b"));
        }
        if let Some(n) = self.files {
            p.push(format!("files={n}"));
        }
        if let Some(b) = self.net {
            p.push(format!("net={b}b"));
        }
        p.join(",")
    }

    pub fn flags(&self) -> Vec<String> {
        if self.is_empty() { vec![] } else { vec!["--limit".into(), self.spec()] }
    }
}

// Pulls every `--limit SPEC` / `--limit=SPEC` before `--` out of argv. Err: the message for a bad one.
pub fn take_limit_flags(argv: &mut Vec<String>) -> Result<Limits, String> {
    let mut l = Limits::default();
    loop {
        let dash = argv.iter().position(|a| a == "--").unwrap_or(argv.len());
        let Some(i) = argv[..dash].iter().position(|a| a == "--limit" || a.starts_with("--limit=")) else { break };
        let flag = argv.remove(i);
        let value = match flag.strip_prefix("--limit=") {
            Some(v) => v.to_string(),
            None if i < dash - 1 => argv.remove(i),
            None => return Err("--limit needs a value: --limit cpu=2,mem=256,time=5,output=64,file=10".into()),
        };
        l.parse(&value)?;
    }
    Ok(l)
}

// Sets the kernel limits (cpu, mem, file) in the child before it starts, together with BREVI_MEM_MB (the
// smaller memory limit wins). If one cannot be set, the program does not start: it never runs unlimited.
#[allow(unused_variables)]
pub fn apply(c: &mut Command, l: &Limits) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let env_mb = std::env::var("BREVI_MEM_MB").ok().and_then(|v| v.trim().parse::<u64>().ok()).filter(|m| *m > 0);
        let mem = match (env_mb, l.mem) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        // Only Linux holds a program to an address-space limit; BREVI_MEM_MB is ignored elsewhere.
        let mem = if cfg!(target_os = "linux") { mem.map(|m| m.saturating_mul(1 << 20)) } else { None };
        let (cpu, file, fds) = (l.cpu, l.file.map(|f| f.saturating_mul(1 << 20)), l.fds);
        // files and net are counted by the program itself (fsscope.rs reads these once).
        for (k, v) in [("BREVI_LIMIT_FILES", l.files), ("BREVI_LIMIT_NET", l.net)] {
            match v {
                Some(n) => drop(c.env(k, n.to_string())),
                None => drop(c.env_remove(k)),
            }
        }
        if mem.is_none() && cpu.is_none() && file.is_none() && fds.is_none() {
            return;
        }
        // RLIMIT_CPU is 0 and RLIMIT_FSIZE 1 on Linux and macOS; RLIMIT_AS is 9 on every Linux architecture
        // Rust targets.
        unsafe {
            c.pre_exec(move || {
                let set = |res: i32, soft: u64, hard: u64| {
                    if setrlimit(res, &[soft, hard]) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                };
                if let Some(b) = mem {
                    set(9, b, b)?;
                }
                // SIGXCPU at the limit; SIGKILL a second later for a program that ignores it.
                if let Some(s) = cpu {
                    set(0, s, s + 1)?;
                }
                if let Some(b) = file {
                    set(1, b, b)?;
                }
                // RLIMIT_NOFILE: 7 on Linux, 8 on macOS and the BSDs
                if let Some(n) = fds {
                    set(if cfg!(target_os = "linux") { 7 } else { 8 }, n, n)?;
                }
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        // Brevi's own file writes check the per-file limit (fsscope.rs); CPython/native writes do not.
        for (k, v) in [("BREVI_LIMIT_FILES", l.files), ("BREVI_LIMIT_NET", l.net)] {
            match v {
                Some(n) => drop(c.env(k, n.to_string())),
                None => drop(c.env_remove(k)),
            }
        }
        match l.file {
            Some(m) => drop(c.env("BREVI_LIMIT_FILE", (m.saturating_mul(1 << 20)).to_string())),
            None => drop(c.env_remove("BREVI_LIMIT_FILE")),
        }
    }
}

#[cfg(unix)]
unsafe extern "C" {
    #[cfg(not(target_os = "linux"))]
    fn setrlimit(resource: i32, rlim: *const [u64; 2]) -> i32;
}

// Which limit stopped a program that ended with this status and stderr, if one did.
pub fn limit_hit(l: &Limits, status: ExitStatus, err_tail: &str) -> Option<&'static str> {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        match status.signal() {
            Some(24) if l.cpu.is_some() => return Some("cpu"),
            Some(25) if l.file.is_some() => return Some("file"),
            Some(6) if l.mem.is_some() && err_tail.contains("memory allocation of") => return Some("mem"),
            _ => {}
        }
    }
    if l.mem.is_some() && status.code() == Some(1) && err_tail.lines().rev().find(|x| !x.trim().is_empty()).is_some_and(|x| x.starts_with("MemoryError")) {
        return Some("mem");
    }
    let _ = (status, err_tail);
    None
}

// The JSON line for a program stopped by a limit (printed on stderr).
pub fn limit_line(l: &Limits, on: &str) -> String {
    let (limit, msg) = match on {
        "cpu" => (format!("{} s", l.cpu.unwrap_or(0)), format!("the program used up its {} s of CPU time", l.cpu.unwrap_or(0))),
        "mem" => (format!("{} MB", l.mem.unwrap_or(0)), format!("the program ran out of its {} MB of memory", l.mem.unwrap_or(0))),
        "time" => {
            let t = l.time.unwrap_or_default().as_secs_f64();
            (format!("{t} s"), format!("the program was stopped after {t} s of wall-clock time"))
        }
        "output" => (format!("{} bytes", l.output.unwrap_or(0)), format!("the program was stopped after writing {} bytes to stdout and stderr", l.output.unwrap_or(0))),
        "threads" => (format!("{} threads", l.threads.unwrap_or(0)), format!("the program started more than {} threads", l.threads.unwrap_or(0))),
        "written" => (format!("{} bytes", l.written.unwrap_or(0)), format!("the program wrote more than {} bytes (files, pipes and sockets; stdout and stderr not counted)", l.written.unwrap_or(0))),
        _ => (format!("{} MB", l.file.unwrap_or(0)), format!("the program tried to write a file over {} MB", l.file.unwrap_or(0))),
    };
    format!("{{\"e\":\"limit\",\"on\":\"{on}\",\"limit\":\"{limit}\",\"msg\":\"{msg}\",\"fix\":\"raise it with --limit {on}=... if the program needs more\"}}")
}

// Runs a program under the limits this process was given (--limit) and returns the exit code to pass on.
// Without time and output limits the program inherits stdout and stderr as before; with an output limit both
// are piped through this process, which counts the bytes and stops the program at the limit.
pub fn run(mut c: Command) -> std::io::Result<i32> {
    run_with(&mut c, &limits())
}

pub fn run_with(c: &mut Command, l: &Limits) -> std::io::Result<i32> {
    use std::io::{Read, Write};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Arc;
    if l.is_empty() {
        return c.status().map(code);
    }
    apply(c, l);
    // stderr's last bytes, to tell a memory limit from another abort.
    let tail = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let over = Arc::new(AtomicBool::new(false));
    let mut pumps = vec![];
    // written subtracts stdout and stderr, so both are counted here
    if l.output.is_some() || l.mem.is_some() || l.written.is_some() {
        c.stderr(std::process::Stdio::piped());
    }
    if l.output.is_some() || l.written.is_some() {
        c.stdout(std::process::Stdio::piped());
    }
    let start = std::time::Instant::now();
    let mut child = c.spawn()?;
    let written = Arc::new(AtomicU64::new(0));
    let cap = l.output.unwrap_or(u64::MAX);
    let streams: Vec<(Box<dyn Read + Send>, bool)> = [child.stdout.take().map(|r| (Box::new(r) as Box<dyn Read + Send>, true)), child.stderr.take().map(|r| (Box::new(r) as Box<dyn Read + Send>, false))].into_iter().flatten().collect();
    for (mut r, is_out) in streams {
        let (written, over, tail) = (written.clone(), over.clone(), tail.clone());
        pumps.push(std::thread::spawn(move || {
            let mut buf = [0u8; 16384];
            loop {
                let n = match r.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                let before = written.fetch_add(n as u64, Ordering::SeqCst);
                let room = cap.saturating_sub(before).min(n as u64) as usize;
                let chunk = &buf[..room];
                if is_out {
                    let mut o = std::io::stdout().lock();
                    let _ = o.write_all(chunk).and_then(|_| o.flush());
                } else {
                    let _ = std::io::stderr().write_all(chunk);
                    let mut t = tail.lock().unwrap_or_else(|e| e.into_inner());
                    t.extend_from_slice(chunk);
                    let len = t.len();
                    if len > 4096 {
                        t.drain(..len - 4096);
                    }
                }
                if room < n {
                    over.store(true, Ordering::SeqCst);
                    break;
                }
            }
        }));
    }
    let mut nap = std::time::Duration::from_micros(100);
    let mut stopped = None;
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if stopped.is_none() {
            if over.load(Ordering::SeqCst) {
                stopped = Some("output");
            } else if l.time.is_some_and(|t| start.elapsed() >= t) {
                stopped = Some("time");
            } else if let Some(on) = over_counts(child.id(), l, written.load(Ordering::SeqCst)) {
                stopped = Some(on);
            }
            if stopped.is_some() {
                let _ = child.kill();
                continue;
            }
        }
        std::thread::sleep(nap);
        nap = (nap * 2).min(std::time::Duration::from_millis(5));
    };
    // The pumps end at the program's end of output; one held open by something it started is not waited for.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        pumps.into_iter().for_each(|p| {
            let _ = p.join();
        });
        let _ = tx.send(());
    });
    let _ = rx.recv_timeout(std::time::Duration::from_secs(2));
    // A program that wrote past the limit just as it ended is still over it.
    if stopped.is_none() && over.load(Ordering::SeqCst) {
        stopped = Some("output");
    }
    let tail = String::from_utf8_lossy(&tail.lock().unwrap_or_else(|e| e.into_inner())).to_string();
    if let Some(on) = stopped.or_else(|| limit_hit(l, status, &tail)) {
        let _ = std::io::stdout().flush();
        eprintln!("{}", limit_line(l, on));
        return Ok(LIMIT_EXIT);
    }
    Ok(code(status))
}

// Windows: committed memory (pagefile usage) and user+kernel CPU time, polled while the program runs.
#[cfg(windows)]
pub fn over_counts(pid: u32, l: &Limits, _piped: u64) -> Option<&'static str> {
    use std::ffi::c_void;
    #[repr(C)]
    struct Pmc {
        cb: u32,
        page_fault_count: u32,
        peak_working_set: usize,
        working_set: usize,
        quota_peak_paged: usize,
        quota_paged: usize,
        quota_peak_nonpaged: usize,
        quota_nonpaged: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }
    #[repr(C)]
    struct Ft {
        lo: u32,
        hi: u32,
    }
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn CloseHandle(h: *mut c_void) -> i32;
        fn K32GetProcessMemoryInfo(h: *mut c_void, p: *mut Pmc, cb: u32) -> i32;
        fn GetProcessTimes(h: *mut c_void, c: *mut Ft, e: *mut Ft, k: *mut Ft, u: *mut Ft) -> i32;
    }
    if l.mem.is_none() && l.cpu.is_none() {
        return None;
    }
    let h = unsafe { OpenProcess(0x1000, 0, pid) };
    if h.is_null() {
        return None;
    }
    let mut out = None;
    if let Some(m) = l.mem {
        let mut pmc = Pmc { cb: std::mem::size_of::<Pmc>() as u32, page_fault_count: 0, peak_working_set: 0, working_set: 0, quota_peak_paged: 0, quota_paged: 0, quota_peak_nonpaged: 0, quota_nonpaged: 0, pagefile_usage: 0, peak_pagefile_usage: 0 };
        if unsafe { K32GetProcessMemoryInfo(h, &mut pmc, pmc.cb) } != 0 && (pmc.pagefile_usage as u64) > m.saturating_mul(1 << 20) {
            out = Some("mem");
        }
    }
    if out.is_none() {
        if let Some(c) = l.cpu {
            let mut p = Ft { lo: 0, hi: 0 };
            let mut e = Ft { lo: 0, hi: 0 };
            let mut k = Ft { lo: 0, hi: 0 };
            let mut u = Ft { lo: 0, hi: 0 };
            if unsafe { GetProcessTimes(h, &mut p, &mut e, &mut k, &mut u) } != 0 {
                let t = |f: &Ft| ((f.hi as u64) << 32 | f.lo as u64) as u64;
                if (t(&k) + t(&u)) / 10_000_000 > c {
                    out = Some("cpu");
                }
            }
        }
    }
    unsafe { CloseHandle(h) };
    out
}

// Linux: whether the running program is over its threads or written limit, from the kernel's counts.
#[cfg(not(windows))]
pub fn over_counts(pid: u32, l: &Limits, piped: u64) -> Option<&'static str> {
    if l.threads.is_none() && l.written.is_none() {
        return None;
    }
    let field = |file: &str, key: &str| -> Option<u64> {
        let t = std::fs::read_to_string(format!("/proc/{pid}/{file}")).ok()?;
        t.lines().find_map(|x| x.strip_prefix(key)).and_then(|v| v.trim().parse().ok())
    };
    if let Some(n) = l.threads {
        if field("status", "Threads:").is_some_and(|t| t > n) {
            return Some("threads");
        }
    }
    if let Some(b) = l.written {
        if field("io", "wchar:").is_some_and(|w| w.saturating_sub(piped) > b) {
            return Some("written");
        }
    }
    None
}

// The exit code to pass on for a program's status. A program killed by a signal has no code: say what
// happened on stderr (one JSON line, like other runtime errors) and use 128 + the signal, as shells do.
pub fn code(status: ExitStatus) -> i32 {
    if let Some(c) = status.code() {
        return c;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            let msg = match sig {
                9 => "the program was killed (signal 9): out of memory, or stopped from outside".to_string(),
                11 | 7 => format!("the program crashed (signal {sig}): often recursion too deep for the stack"),
                6 => "the program aborted (signal 6)".to_string(),
                s => format!("the program was stopped by signal {s}"),
            };
            eprintln!("{{\"e\":\"signal\",\"msg\":\"{msg}\"}}");
            return 128 + sig;
        }
    }
    1
}

// Puts the child in its own process group, so `stop` reaches everything it starts (rustc, the built
// program, python3).
pub fn own_group(c: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x200;
        c.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }
}

// Kills a child started with own_group and everything under it.
pub fn stop(child: &mut Child) {
    stop_pid(child.id());
    let _ = child.kill();
}

pub fn stop_pid(pid: u32) {
    #[cfg(unix)]
    unsafe {
        kill(-(pid as i32), 9);
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

// Whether a process is still running (unix; elsewhere assumed so).
pub fn alive(pid: u32) -> bool {
    #[cfg(unix)]
    unsafe {
        kill(pid as i32, 0) == 0 || std::io::Error::last_os_error().raw_os_error() != Some(3)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn parses_limits() {
        let mut l = Limits::default();
        #[cfg(not(windows))]
        let spec = "cpu=1.2,time=500ms,output=2kb,file=3";
        #[cfg(windows)]
        let spec = "cpu=1.2,time=500ms,output=2kb";
        l.parse(spec).unwrap();
        assert_eq!(l.cpu, Some(2));
        assert_eq!(l.time, Some(Duration::from_millis(500)));
        assert_eq!(l.output, Some(2048));
        #[cfg(not(windows))]
        assert_eq!(l.file, Some(3));
        #[cfg(windows)]
        assert!(Limits::default().parse("file=3").is_err());
        let mut back = Limits::default();
        back.parse(&l.spec()).unwrap();
        assert_eq!(back, l);
        let mut o = Limits::default();
        o.parse("output=64").unwrap();
        assert_eq!(o.output, Some(64 << 10));
        for bad in ["cpu", "cpu=0", "cpu=-1", "time=x", "disk=1", "mem=nan", "time=inf", "threads=0", "fds=x", "files=1.5", "net=0"] {
            assert!(Limits::default().parse(bad).is_err(), "{bad} should be refused");
        }
        #[cfg(target_os = "linux")]
        {
            let mut m = Limits::default();
            m.parse("mem=1gb").unwrap();
            assert_eq!(m.mem, Some(1024));
        }
    }

    // threads and written are read from /proc while the program runs.
    #[cfg(target_os = "linux")]
    #[test]
    fn holds_threads_and_written() {
        if Command::new("python3").arg("-c").arg("1").status().map_or(true, |s| !s.success()) {
            return;
        }
        let py = |code: &str| {
            let mut c = Command::new("python3");
            c.arg("-c").arg(code).stdout(std::process::Stdio::null());
            c
        };
        let mut l = Limits::default();
        l.parse("threads=4,time=20").unwrap();
        let threads = "import threading, time\nfor _ in range(8): threading.Thread(target=time.sleep, args=(3,)).start()\n";
        assert_eq!(run_with(&mut py(threads), &l).unwrap(), LIMIT_EXIT);
        let mut few = Limits::default();
        few.parse("threads=16,time=20").unwrap();
        assert_eq!(run_with(&mut py("import threading; t = threading.Thread(target=print); t.start(); t.join()"), &few).unwrap(), 0);
        let dir = std::env::temp_dir().join(format!("brevi-written-{}", std::process::id()));
        let mut w = Limits::default();
        w.parse("written=1,time=20").unwrap();
        let big = format!("import time\nfor i in range(40):\n    open({:?}, 'w').write('x' * 100000)\n    time.sleep(0.01)\n", dir.to_string_lossy());
        assert_eq!(run_with(&mut py(&big), &w).unwrap(), LIMIT_EXIT);
        let _ = std::fs::remove_file(&dir);
    }

    #[test]
    fn takes_limit_flags_before_dashes() {
        let mut argv: Vec<String> = ["run", "x.py", "--limit", "cpu=1", "--limit=time=2", "--", "--limit", "cpu=9"].iter().map(|s| s.to_string()).collect();
        let l = take_limit_flags(&mut argv).unwrap();
        assert_eq!(l.cpu, Some(1));
        assert_eq!(l.time, Some(Duration::from_secs(2)));
        assert_eq!(argv, ["run", "x.py", "--", "--limit", "cpu=9"]);
    }
}
