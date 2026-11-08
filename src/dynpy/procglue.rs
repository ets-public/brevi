// The system side of _brevi_proc (rt_net.rs), pasted into a program that starts child processes with Brevi's
// asyncio (create_subprocess_exec, create_subprocess_shell): std::process with pipes whose parent ends are
// non-blocking, for the event loop to wait on with poll.
mod brevi_procglue {
    use brevi_dyn::{NetErr, ProcOps};
    use std::collections::HashMap;
    use std::os::raw::{c_int, c_void};
    use std::os::unix::io::IntoRawFd;
    use std::os::unix::process::ExitStatusExt;
    use std::process::{Child, Command, Stdio};
    use std::sync::Mutex;

    #[repr(C)]
    struct PollFd {
        fd: c_int,
        events: i16,
        revents: i16,
    }

    extern "C" {
        fn read(fd: c_int, buf: *mut c_void, n: usize) -> isize;
        fn write(fd: c_int, buf: *const c_void, n: usize) -> isize;
        fn close(fd: c_int) -> c_int;
        fn poll(fds: *mut PollFd, n: u64, timeout: c_int) -> c_int;
        fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
        fn kill(pid: c_int, sig: c_int) -> c_int;
    }

    const F_GETFL: c_int = 3;
    const F_SETFL: c_int = 4;
    const O_NONBLOCK: c_int = if cfg!(target_os = "macos") { 4 } else { 2048 };

    static CHILDREN: Mutex<Option<HashMap<i64, Child>>> = Mutex::new(None);
    static NEXT: Mutex<i64> = Mutex::new(0);

    fn errno() -> i32 {
        std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
    }

    fn strerror(e: i32) -> String {
        let s = std::io::Error::from_raw_os_error(e).to_string();
        match s.find(" (os error") {
            Some(i) => s[..i].to_string(),
            None => s,
        }
    }

    fn err(e: i32) -> NetErr {
        NetErr { kind: kind(e), msg: format!("[Errno {e}] {}", strerror(e)) }
    }

    fn kind(e: i32) -> &'static str {
        match e {
            2 => "FileNotFoundError",
            1 | 13 => "PermissionError",
            20 => "NotADirectoryError",
            21 => "IsADirectoryError",
            _ => "OSError",
        }
    }

    fn nonblock(fd: c_int) {
        unsafe {
            let f = fcntl(fd, F_GETFL);
            fcntl(fd, F_SETFL, f | O_NONBLOCK);
        }
    }

    fn op_spawn(argv: &[String], cwd: Option<&str>, env: Option<&[(String, String)]>, modes: [i64; 3]) -> Result<(i64, i64, [i64; 3]), NetErr> {
        let mut cmd = Command::new(&argv[0]);
        cmd.args(&argv[1..]);
        if let Some(d) = cwd {
            cmd.current_dir(d);
        }
        if let Some(e) = env {
            cmd.env_clear();
            cmd.envs(e.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        }
        let io = |e: std::io::Error| err(e.raw_os_error().unwrap_or(5));
        let mut fds = [-1i64; 3];
        // stdin
        match modes[0] {
            -1 => {
                let (r, w) = std::io::pipe().map_err(io)?;
                cmd.stdin(Stdio::from(r));
                fds[0] = w.into_raw_fd() as i64;
            }
            -3 => {
                cmd.stdin(Stdio::null());
            }
            _ => {
                cmd.stdin(Stdio::inherit());
            }
        }
        let mut out_w = None;
        match modes[1] {
            -1 => {
                let (r, w) = std::io::pipe().map_err(io)?;
                out_w = Some(w.try_clone().map_err(io)?);
                cmd.stdout(Stdio::from(w));
                fds[1] = r.into_raw_fd() as i64;
            }
            -3 => {
                cmd.stdout(Stdio::null());
            }
            _ => {
                cmd.stdout(Stdio::inherit());
            }
        }
        match modes[2] {
            -1 => {
                let (r, w) = std::io::pipe().map_err(io)?;
                cmd.stderr(Stdio::from(w));
                fds[2] = r.into_raw_fd() as i64;
            }
            -2 => match out_w.take() {
                Some(w) => {
                    cmd.stderr(Stdio::from(w));
                }
                None => {
                    // stdout not a pipe: stderr goes where stdout goes
                    let o = unsafe { fcntl(1, if cfg!(target_os = "macos") { 67 } else { 1030 }) };
                    if o >= 0 {
                        use std::os::unix::io::FromRawFd;
                        cmd.stderr(unsafe { Stdio::from_raw_fd(o) });
                    }
                }
            },
            -3 => {
                cmd.stderr(Stdio::null());
            }
            _ => {
                cmd.stderr(Stdio::inherit());
            }
        }
        drop(out_w);
        let spawned = cmd.spawn();
        drop(cmd);
        let child = match spawned {
            Ok(c) => c,
            Err(e) => {
                for f in fds.iter().filter(|f| **f >= 0) {
                    unsafe { close(*f as c_int) };
                }
                let n = e.raw_os_error().unwrap_or(5);
                let mut x = err(n);
                // CPython names the program (or the folder) it could not start
                if n == 2 || n == 13 || n == 20 {
                    let p = if n == 2 && cwd.is_some_and(|d| !std::path::Path::new(d).is_dir()) { cwd.unwrap() } else { &argv[0] };
                    x.msg = format!("{}: '{}'", x.msg, p);
                }
                return Err(x);
            }
        };
        for f in fds.iter().filter(|f| **f >= 0) {
            nonblock(*f as c_int);
        }
        let pid = child.id() as i64;
        let id = {
            let mut n = NEXT.lock().unwrap();
            *n += 1;
            *n
        };
        CHILDREN.lock().unwrap().get_or_insert_with(HashMap::new).insert(id, child);
        Ok((id, pid, fds))
    }

    fn op_read(fd: i64, n: usize) -> Result<Option<Vec<u8>>, NetErr> {
        let mut b = vec![0u8; n];
        loop {
            let k = unsafe { read(fd as c_int, b.as_mut_ptr() as *mut c_void, n) };
            if k < 0 {
                let e = errno();
                if e == 4 {
                    continue;
                }
                if e == 11 || e == 35 {
                    return Ok(None);
                }
                return Err(err(e));
            }
            b.truncate(k as usize);
            return Ok(Some(b));
        }
    }

    fn op_write(fd: i64, b: &[u8]) -> Result<i64, NetErr> {
        loop {
            let k = unsafe { write(fd as c_int, b.as_ptr() as *const c_void, b.len()) };
            if k < 0 {
                let e = errno();
                if e == 4 {
                    continue;
                }
                if e == 11 || e == 35 {
                    return Ok(0);
                }
                if e == 32 {
                    return Err(NetErr { kind: "BrokenPipeError", msg: format!("[Errno 32] {}", strerror(32)) });
                }
                return Err(err(e));
            }
            return Ok(k as i64);
        }
    }

    fn op_close(fd: i64) {
        unsafe { close(fd as c_int) };
    }

    fn op_wait(id: i64, block: bool) -> Result<Option<i64>, NetErr> {
        let mut g = CHILDREN.lock().unwrap();
        let c = match g.as_mut().and_then(|m| m.get_mut(&id)) {
            Some(c) => c,
            None => return Err(NetErr { kind: "ProcessLookupError", msg: "[Errno 3] No such process".into() }),
        };
        let st = if block { c.wait().map(Some) } else { c.try_wait() };
        match st {
            Ok(Some(s)) => Ok(Some(match s.code() {
                Some(c) => c as i64,
                None => -(s.signal().unwrap_or(0) as i64),
            })),
            Ok(None) => Ok(None),
            Err(e) => Err(err(e.raw_os_error().unwrap_or(10))),
        }
    }

    fn op_signal(pid: i64, sig: i64) -> Result<(), NetErr> {
        if unsafe { kill(pid as c_int, sig as c_int) } < 0 {
            let e = errno();
            return Err(if e == 3 { NetErr { kind: "ProcessLookupError", msg: format!("[Errno 3] {}", strerror(3)) } } else { err(e) });
        }
        Ok(())
    }

    fn op_poll(fds: &[(i64, i64)], timeout: f64) -> Result<Vec<i64>, NetErr> {
        let mut p: Vec<PollFd> = fds.iter().map(|&(fd, ev)| PollFd { fd: fd as c_int, events: ev as i16, revents: 0 }).collect();
        let ms = if timeout < 0.0 { -1 } else { (timeout * 1000.0).ceil() as c_int };
        loop {
            let r = unsafe { poll(p.as_mut_ptr(), p.len() as u64, ms) };
            if r < 0 {
                let e = errno();
                if e == 4 {
                    continue;
                }
                return Err(err(e));
            }
            return Ok(p.iter().map(|x| x.revents as i64).collect());
        }
    }

    pub fn ops() -> ProcOps {
        ProcOps { spawn: op_spawn, read: op_read, write: op_write, close: op_close, wait: op_wait, signal: op_signal, poll: op_poll }
    }
}
