// The system side of _brevi_stdio (rt_net.rs), pasted into a program that imports Brevi's mcp: fds 0 and 1
// claimed for the protocol as the SDK's mcp.server.stdio does it.
mod brevi_stdioglue {
    use brevi_dyn::{NetErr, StdioOps};
    use std::os::raw::{c_char, c_int, c_void};

    #[repr(C)]
    struct PollFd {
        fd: c_int,
        events: i16,
        revents: i16,
    }

    extern "C" {
        fn dup(fd: c_int) -> c_int;
        fn dup2(a: c_int, b: c_int) -> c_int;
        fn open(p: *const c_char, flags: c_int, ...) -> c_int;
        fn close(fd: c_int) -> c_int;
        fn read(fd: c_int, buf: *mut c_void, n: usize) -> isize;
        fn write(fd: c_int, buf: *const c_void, n: usize) -> isize;
        fn poll(fds: *mut PollFd, n: u64, timeout: c_int) -> c_int;
        fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
    }

    const F_DUPFD_CLOEXEC: c_int = if cfg!(target_os = "macos") { 67 } else { 1030 };

    static mut SAVED: (c_int, c_int) = (-1, -1);

    fn errno() -> i32 {
        std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
    }

    fn err(e: i32) -> NetErr {
        NetErr { kind: "OSError", msg: format!("[Errno {e}] {}", std::io::Error::from_raw_os_error(e)) }
    }

    fn op_claim() -> Result<(i64, i64), NetErr> {
        unsafe {
            let i = fcntl(0, F_DUPFD_CLOEXEC, 3);
            let o = fcntl(1, F_DUPFD_CLOEXEC, 3);
            if i < 0 || o < 0 {
                return Err(err(errno()));
            }
            let null = open(b"/dev/null\0".as_ptr() as *const c_char, 0);
            if null >= 0 {
                dup2(null, 0);
                close(null);
            }
            let e = dup(2);
            if e >= 0 {
                dup2(e, 1);
                close(e);
            }
            SAVED = (i, o);
            Ok((i as i64, o as i64))
        }
    }

    fn op_release() {
        unsafe {
            let (i, o) = SAVED;
            if i >= 0 {
                dup2(i, 0);
            }
            if o >= 0 {
                dup2(o, 1);
            }
        }
    }

    fn op_read(fd: i64, n: usize) -> Result<Vec<u8>, NetErr> {
        let mut b = vec![0u8; n];
        loop {
            let k = unsafe { read(fd as c_int, b.as_mut_ptr() as *mut c_void, n) };
            if k < 0 {
                let e = errno();
                if e == 4 {
                    continue;
                }
                return Err(err(e));
            }
            b.truncate(k as usize);
            return Ok(b);
        }
    }

    fn op_write(fd: i64, b: &[u8]) -> Result<(), NetErr> {
        let mut at = 0;
        while at < b.len() {
            let k = unsafe { write(fd as c_int, b[at..].as_ptr() as *const c_void, b.len() - at) };
            if k < 0 {
                let e = errno();
                if e == 4 || e == 11 || e == 35 {
                    continue;
                }
                return Err(err(e));
            }
            at += k as usize;
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

    pub fn ops() -> StdioOps {
        StdioOps { claim: op_claim, release: op_release, read: op_read, write: op_write, poll: op_poll }
    }
}
