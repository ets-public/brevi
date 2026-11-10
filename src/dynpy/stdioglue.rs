// The system side of _brevi_stdio (rt_net.rs), pasted into a program that imports Brevi's mcp: fds 0 and 1
// claimed for the protocol as the SDK's mcp.server.stdio does it.
//
// Unix duplicates fd 0 and 1 for the protocol (close-on-exec), then points fd 0 at /dev/null and fd 1 at stderr
// so that print() cannot corrupt the stream, and puts both back on release. Windows has no fds: there the
// protocol's ends are duplicated kernel handles (ReadFile/WriteFile on them), and "fd 0" and "fd 1" are the
// process's standard handles, which Rust's std reads and writes through on every call: SetStdHandle moves them
// to a NUL handle (an empty stdin) and to the stderr handle, and release puts the saved handles back.

#[cfg(unix)]
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

#[cfg(windows)]
mod brevi_stdioglue {
    use brevi_dyn::{NetErr, StdioOps};
    use std::os::raw::c_void;
    use std::sync::atomic::{AtomicUsize, Ordering};

    type Handle = *mut c_void;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(which: u32) -> Handle;
        fn SetStdHandle(which: u32, h: Handle) -> i32;
        fn GetCurrentProcess() -> Handle;
        fn DuplicateHandle(proc_src: Handle, src: Handle, proc_dst: Handle, dst: *mut Handle, access: u32, inherit: i32, options: u32) -> i32;
        fn CreateFileW(name: *const u16, access: u32, share: u32, sa: *mut c_void, disposition: u32, flags: u32, template: Handle) -> Handle;
        fn ReadFile(h: Handle, buf: *mut c_void, n: u32, got: *mut u32, ov: *mut c_void) -> i32;
        fn WriteFile(h: Handle, buf: *const c_void, n: u32, got: *mut u32, ov: *mut c_void) -> i32;
        fn WaitForMultipleObjects(n: u32, hs: *const Handle, wait_all: i32, ms: u32) -> u32;
        fn WaitForSingleObject(h: Handle, ms: u32) -> u32;
        fn PeekNamedPipe(h: Handle, buf: *mut c_void, n: u32, read: *mut u32, avail: *mut u32, left: *mut u32) -> i32;
        fn CloseHandle(h: Handle) -> i32;
        fn GetLastError() -> u32;
    }

    // STD_INPUT_HANDLE / STD_OUTPUT_HANDLE / STD_ERROR_HANDLE: (DWORD)-10, -11, -12
    const STD_INPUT: u32 = 0xFFFF_FFF6;
    const STD_OUTPUT: u32 = 0xFFFF_FFF5;
    const STD_ERROR: u32 = 0xFFFF_FFF4;

    const DUPLICATE_SAME_ACCESS: u32 = 0x2;
    const GENERIC_READ: u32 = 0x8000_0000;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const OPEN_EXISTING: u32 = 0x3;

    const INFINITE: u32 = 0xFFFF_FFFF;
    const WAIT_TIMEOUT: u32 = 0x102;
    const WAIT_FAILED: u32 = 0xFFFF_FFFF;
    // the most handles one WaitForMultipleObjects call takes
    const MAXIMUM_WAIT_OBJECTS: usize = 64;

    // INVALID_HANDLE_VALUE, which GetStdHandle gives when a standard handle was never set (a program started
    // with none, e.g. from a GUI parent): there is nothing to claim for the protocol
    const INVALID: usize = usize::MAX;

    // ERROR_INVALID_HANDLE / ERROR_BROKEN_PIPE / ERROR_NO_DATA
    const ERROR_INVALID_HANDLE: u32 = 6;
    const ERROR_BROKEN_PIPE: u32 = 109;
    const ERROR_NO_DATA: u32 = 232;

    // what the standard handles were before claim (0: not claimed), and the NUL handle fd 0 was moved to
    static SAVED_IN: AtomicUsize = AtomicUsize::new(0);
    static SAVED_OUT: AtomicUsize = AtomicUsize::new(0);
    static NUL: AtomicUsize = AtomicUsize::new(0);

    fn strerror(e: u32) -> String {
        let s = std::io::Error::from_raw_os_error(e as i32).to_string();
        match s.find(" (os error") {
            Some(i) => s[..i].to_string(),
            None => s,
        }
    }

    fn err(e: u32) -> NetErr {
        let kind = match e {
            32 | ERROR_BROKEN_PIPE | ERROR_NO_DATA => "BrokenPipeError",
            _ => "OSError",
        };
        NetErr { kind, msg: format!("[Errno {e}] {}", strerror(e)) }
    }

    fn last() -> NetErr {
        err(unsafe { GetLastError() })
    }

    fn op_claim() -> Result<(i64, i64), NetErr> {
        unsafe {
            let cur = GetCurrentProcess();
            let inp = GetStdHandle(STD_INPUT);
            let outp = GetStdHandle(STD_OUTPUT);
            if inp.is_null() || inp as usize == INVALID || outp.is_null() || outp as usize == INVALID {
                return Err(err(ERROR_INVALID_HANDLE));
            }
            // private handles for the protocol: the client's pipe ends, exactly as they are
            let mut pi: Handle = std::ptr::null_mut();
            let mut po: Handle = std::ptr::null_mut();
            if DuplicateHandle(cur, inp, cur, &mut pi, 0, 0, DUPLICATE_SAME_ACCESS) == 0 {
                return Err(last());
            }
            if DuplicateHandle(cur, outp, cur, &mut po, 0, 0, DUPLICATE_SAME_ACCESS) == 0 {
                return Err(last());
            }
            // what fd 0 becomes: NUL, which reads as an empty stdin (the compiler asked for /dev/null)
            let nul: Vec<u16> = "NUL\0".encode_utf16().collect();
            let nh = CreateFileW(nul.as_ptr(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null_mut(), OPEN_EXISTING, 0, std::ptr::null_mut());
            if !nh.is_null() && nh as usize != INVALID {
                SetStdHandle(STD_INPUT, nh);
                NUL.store(nh as usize, Ordering::SeqCst);
            }
            // what fd 1 becomes: stderr, so print() during the session goes there
            let eh = GetStdHandle(STD_ERROR);
            if !eh.is_null() && eh as usize != INVALID {
                SetStdHandle(STD_OUTPUT, eh);
            }
            SAVED_IN.store(inp as usize, Ordering::SeqCst);
            SAVED_OUT.store(outp as usize, Ordering::SeqCst);
            Ok((pi as i64, po as i64))
        }
    }

    fn op_release() {
        unsafe {
            let i = SAVED_IN.swap(0, Ordering::SeqCst);
            let o = SAVED_OUT.swap(0, Ordering::SeqCst);
            if i != 0 {
                SetStdHandle(STD_INPUT, i as Handle);
            }
            if o != 0 {
                SetStdHandle(STD_OUTPUT, o as Handle);
            }
            let n = NUL.swap(0, Ordering::SeqCst);
            if n != 0 {
                CloseHandle(n as Handle);
            }
        }
    }

    fn op_read(fd: i64, n: usize) -> Result<Vec<u8>, NetErr> {
        let mut b = vec![0u8; n];
        loop {
            let mut got: u32 = 0;
            let ok = unsafe { ReadFile(fd as Handle, b.as_mut_ptr() as *mut c_void, n.min(u32::MAX as usize) as u32, &mut got, std::ptr::null_mut()) };
            if ok == 0 {
                let e = unsafe { GetLastError() };
                if e == ERROR_BROKEN_PIPE || e == ERROR_NO_DATA {
                    // the client closed its end: the empty read that is EOF to the caller
                    b.truncate(0);
                    return Ok(b);
                }
                if e == 87 && got == 0 {
                    // ERROR_INVALID_PARAMETER with nothing read: a zero-length request
                    return Ok(b);
                }
                return Err(err(e));
            }
            b.truncate(got as usize);
            return Ok(b);
        }
    }

    fn op_write(fd: i64, b: &[u8]) -> Result<(), NetErr> {
        let mut at = 0;
        while at < b.len() {
            let mut got: u32 = 0;
            let ok = unsafe { WriteFile(fd as Handle, b[at..].as_ptr() as *const c_void, (b.len() - at).min(u32::MAX as usize) as u32, &mut got, std::ptr::null_mut()) };
            if ok == 0 {
                return Err(last());
            }
            if got == 0 {
                // a synchronous pipe write that took nothing: report it as broken rather than spin
                return Err(err(ERROR_BROKEN_PIPE));
            }
            at += got as usize;
        }
        Ok(())
    }

    fn op_poll(fds: &[(i64, i64)], timeout: f64) -> Result<Vec<i64>, NetErr> {
        // WaitForMultipleObjects signals an anonymous-pipe read handle whenever the pipe is open, even with no
        // bytes (measured), so peeking is what tells readable from empty. PeekNamedPipe's available bytes are
        // the readiness; a broken pipe is EOF (readable now). Handles that are not pipes fall back to the wait.
        // Sleep 1 to 10 ms between peeks so asyncio timers still run while stdin is idle.
        let start = std::time::Instant::now();
        let total: Option<u128> = if timeout < 0.0 { None } else { Some((timeout * 1000.0).ceil().max(0.0) as u128) };
        loop {
            let mut rev = vec![0i64; fds.len()];
            let mut ready = false;
            let mut wait_handles: Vec<Handle> = vec![];
            let mut wait_idx: Vec<usize> = vec![];
            for (i, &(fd, ev)) in fds.iter().enumerate() {
                if ev & 1 == 0 {
                    // writable (4) or anything else: no waiting needed
                    rev[i] = ev;
                    continue;
                }
                let h = fd as usize as Handle;
                let mut avail: u32 = 0;
                let ok = unsafe { PeekNamedPipe(h, std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut avail, std::ptr::null_mut()) };
                if ok != 0 {
                    if avail > 0 {
                        rev[i] |= 1;
                        ready = true;
                    }
                } else {
                    let e = unsafe { GetLastError() };
                    if e == ERROR_BROKEN_PIPE || e == ERROR_NO_DATA {
                        // the client closed its end: readable, and op_read returns the EOF
                        rev[i] |= 1;
                        ready = true;
                    } else if e == ERROR_INVALID_HANDLE {
                        wait_handles.push(h);
                        wait_idx.push(i);
                    } else {
                        return Err(err(e));
                    }
                }
            }
            if ready {
                return Ok(rev);
            }
            let gone = start.elapsed().as_millis();
            if let Some(t) = total {
                if gone >= t {
                    return Ok(rev);
                }
            }
            if !wait_handles.is_empty() {
                let ms = match total {
                    None => 10u32,
                    Some(t) => (t - gone).min(10) as u32,
                };
                let r = unsafe { WaitForMultipleObjects(wait_handles.len() as u32, wait_handles.as_ptr(), 0, ms) };
                if r == WAIT_FAILED {
                    return Err(last());
                }
                if (r as usize) < wait_handles.len() {
                    rev[wait_idx[r as usize]] |= 1;
                    return Ok(rev);
                }
            }
            let left = match total {
                None => 10,
                Some(t) => (t - start.elapsed().as_millis()).min(10),
            };
            std::thread::sleep(std::time::Duration::from_millis(left.max(1) as u64));
        }
    }

    pub fn ops() -> StdioOps {
        StdioOps { claim: op_claim, release: op_release, read: op_read, write: op_write, poll: op_poll }
    }
}
