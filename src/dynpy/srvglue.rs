// The system side of _brevi_srv (rt_net.rs), pasted into a program that serves MCP over HTTP with Brevi's mcp
// (mcp.run(transport="streamable-http") or "sse"): a listening TCP socket and the connections it accepts, all
// non-blocking, for the event loop to wait on with poll. Under --sandbox it listens only on a host the caller
// granted (SRV_ANY / SRV_HOSTS, set by the compiler).
//
// Unix keeps the kernel's sockets as plain fds. Windows has no fds: there a socket id is the SOCKET value, the
// listener and the accepted connections stay in two tables of owned TcpListener/TcpStream (dropping one closes
// it), and the event loop's poll is WSAPoll, whose bits for "readable"/"writable" (POLLRDNORM/POLLWRNORM) are
// not Unix poll's (1/4), so they are translated at the boundary.

#[cfg(unix)]
mod brevi_srvglue {
    use brevi_dyn::{NetErr, SrvOps};
    use std::collections::HashMap;
    use std::net::{TcpListener, ToSocketAddrs};
    use std::os::raw::{c_int, c_void};
    use std::os::unix::io::{AsRawFd, IntoRawFd};
    use std::sync::Mutex;

    #[repr(C)]
    struct PollFd {
        fd: c_int,
        events: i16,
        revents: i16,
    }

    extern "C" {
        fn read(fd: c_int, buf: *mut c_void, n: usize) -> isize;
        fn send(fd: c_int, buf: *const c_void, n: usize, flags: c_int) -> isize;
        fn close(fd: c_int) -> c_int;
        fn poll(fds: *mut PollFd, n: u64, timeout: c_int) -> c_int;
        fn getentropy(buf: *mut c_void, n: usize) -> c_int;
    }

    // MSG_NOSIGNAL: a write to a closed connection is an error, not SIGPIPE (macOS has SO_NOSIGPIPE instead,
    // and Rust programs ignore SIGPIPE anyway)
    const NOSIG: c_int = if cfg!(target_os = "linux") { 0x4000 } else { 0 };

    static LISTENERS: Mutex<Option<HashMap<i64, TcpListener>>> = Mutex::new(None);

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
        let kind = match e {
            32 => "BrokenPipeError",
            104 | 54 => "ConnectionResetError",
            13 | 1 => "PermissionError",
            _ => "OSError",
        };
        NetErr { kind, msg: format!("[Errno {e}] {}", strerror(e)) }
    }

    fn io(e: std::io::Error) -> NetErr {
        match e.raw_os_error() {
            Some(c) => err(c),
            None => NetErr { kind: "OSError", msg: e.to_string() },
        }
    }

    fn op_listen(host: &str, port: u16) -> Result<(i64, i64), NetErr> {
        let h = host.trim_start_matches('[').trim_end_matches(']');
        if !super::SRV_ANY && !super::SRV_HOSTS.iter().any(|x| x.eq_ignore_ascii_case(h)) {
            return Err(NetErr {
                kind: "PermissionError",
                msg: format!("brevi sandbox: the caller did not grant the network to {host}, so the server cannot listen on it (--allow-net {host}, or --allow net)"),
            });
        }
        let addrs: Vec<_> = (host, port).to_socket_addrs().map_err(|e| NetErr { kind: "OSError", msg: format!("[Errno -2] {e}") })?.collect();
        let mut last = None;
        for a in addrs {
            match TcpListener::bind(a) {
                Ok(l) => {
                    l.set_nonblocking(true).map_err(io)?;
                    let p = l.local_addr().map(|x| x.port() as i64).unwrap_or(port as i64);
                    let fd = l.as_raw_fd() as i64;
                    LISTENERS.lock().unwrap().get_or_insert_with(HashMap::new).insert(fd, l);
                    return Ok((fd, p));
                }
                Err(e) => last = Some(e),
            }
        }
        Err(match last {
            Some(e) => io(e),
            None => NetErr { kind: "OSError", msg: format!("no address for {host}") },
        })
    }

    fn op_accept(fd: i64) -> Result<Option<(i64, String)>, NetErr> {
        let g = LISTENERS.lock().unwrap();
        let l = match g.as_ref().and_then(|m| m.get(&fd)) {
            Some(l) => l,
            None => return Err(err(9)),
        };
        loop {
            match l.accept() {
                Ok((s, peer)) => {
                    s.set_nonblocking(true).map_err(io)?;
                    let _ = s.set_nodelay(true);
                    return Ok(Some((s.into_raw_fd() as i64, peer.to_string())));
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                // a connection reset before it was accepted: try the next
                Err(e) if matches!(e.raw_os_error(), Some(103) | Some(53) | Some(71)) => continue,
                Err(e) => return Err(io(e)),
            }
        }
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
            let k = unsafe { send(fd as c_int, b.as_ptr() as *const c_void, b.len(), NOSIG) };
            if k < 0 {
                let e = errno();
                if e == 4 {
                    continue;
                }
                if e == 11 || e == 35 {
                    return Ok(0);
                }
                return Err(err(e));
            }
            return Ok(k as i64);
        }
    }

    fn op_close(fd: i64) {
        if let Some(l) = LISTENERS.lock().unwrap().as_mut().and_then(|m| m.remove(&fd)) {
            drop(l);
            return;
        }
        unsafe { close(fd as c_int) };
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

    fn op_random(n: usize) -> Result<Vec<u8>, NetErr> {
        // getentropy (Linux glibc 2.25+, macOS): no file to open, so no file grant needed
        let mut b = vec![0u8; n];
        for c in b.chunks_mut(256) {
            if unsafe { getentropy(c.as_mut_ptr() as *mut c_void, c.len()) } != 0 {
                return Err(err(errno()));
            }
        }
        Ok(b)
    }

    pub fn ops() -> SrvOps {
        SrvOps { listen: op_listen, accept: op_accept, read: op_read, write: op_write, close: op_close, poll: op_poll, random: op_random }
    }
}

#[cfg(windows)]
mod brevi_srvglue {
    use brevi_dyn::{NetErr, SrvOps};
    use std::collections::HashMap;
    use std::io::{ErrorKind, Read, Write};
    use std::net::{TcpListener, TcpStream, ToSocketAddrs};
    use std::os::raw::c_void;
    use std::os::windows::io::AsRawSocket;
    use std::sync::Mutex;

    // WSAPOLLFD: a SOCKET, then the events asked for and those that happened, as Windows' own bits
    #[repr(C)]
    struct PollFd {
        fd: usize,
        events: i16,
        revents: i16,
    }

    #[link(name = "ws2_32")]
    extern "system" {
        fn WSAPoll(fds: *mut PollFd, nfds: u32, timeout: i32) -> i32;
        fn WSAGetLastError() -> i32;
    }

    #[link(name = "bcrypt")]
    extern "system" {
        // BCRYPT_USE_SYSTEM_PREFERRED_RNG with no algorithm handle: the system's random source
        fn BCryptGenRandom(alg: *mut c_void, buf: *mut u8, len: u32, flags: u32) -> i32;
    }

    const SYSTEM_RNG: u32 = 0x2;

    // Winsock's poll bits: Unix poll's 1 (readable) is POLLRDNORM here, and its 4 (writable) is POLLWRNORM
    const POLLRDNORM: i16 = 0x0100;
    const POLLRDBAND: i16 = 0x0200;
    const POLLWRNORM: i16 = 0x0010;
    const POLLERR: i16 = 0x0001;
    const POLLHUP: i16 = 0x0002;
    const POLLNVAL: i16 = 0x0004;

    // WSAEINTR: an interrupted wait (retry, as the Unix glue retries EINTR); WSAENOBUFS: the error reported
    // when WSAPoll fails without setting one
    const EINTR: i32 = 10004;
    const ENOBUF: i32 = 10055;

    static LISTENERS: Mutex<Option<HashMap<i64, TcpListener>>> = Mutex::new(None);
    static CONNS: Mutex<Option<HashMap<i64, TcpStream>>> = Mutex::new(None);

    fn strerror(e: i32) -> String {
        let s = std::io::Error::from_raw_os_error(e).to_string();
        match s.find(" (os error") {
            Some(i) => s[..i].to_string(),
            None => s,
        }
    }

    fn err(e: i32) -> NetErr {
        let kind = match e {
            32 | 109 | 232 => "BrokenPipeError",
            10054 | 10052 => "ConnectionResetError",
            10013 | 1 => "PermissionError",
            _ => "OSError",
        };
        NetErr { kind, msg: format!("[Errno {e}] {}", strerror(e)) }
    }

    fn io(e: std::io::Error) -> NetErr {
        match e.raw_os_error() {
            Some(c) => err(c),
            None => NetErr { kind: "OSError", msg: e.to_string() },
        }
    }

    fn op_listen(host: &str, port: u16) -> Result<(i64, i64), NetErr> {
        let h = host.trim_start_matches('[').trim_end_matches(']');
        if !super::SRV_ANY && !super::SRV_HOSTS.iter().any(|x| x.eq_ignore_ascii_case(h)) {
            return Err(NetErr {
                kind: "PermissionError",
                msg: format!("brevi sandbox: the caller did not grant the network to {host}, so the server cannot listen on it (--allow-net {host}, or --allow net)"),
            });
        }
        let addrs: Vec<_> = (host, port).to_socket_addrs().map_err(|e| NetErr { kind: "OSError", msg: format!("[Errno -2] {e}") })?.collect();
        let mut last = None;
        for a in addrs {
            match TcpListener::bind(a) {
                Ok(l) => {
                    l.set_nonblocking(true).map_err(io)?;
                    let p = l.local_addr().map(|x| x.port() as i64).unwrap_or(port as i64);
                    let fd = l.as_raw_socket() as i64;
                    LISTENERS.lock().unwrap().get_or_insert_with(HashMap::new).insert(fd, l);
                    return Ok((fd, p));
                }
                Err(e) => last = Some(e),
            }
        }
        Err(match last {
            Some(e) => io(e),
            None => NetErr { kind: "OSError", msg: format!("no address for {host}") },
        })
    }

    fn op_accept(fd: i64) -> Result<Option<(i64, String)>, NetErr> {
        let g = LISTENERS.lock().unwrap();
        let l = match g.as_ref().and_then(|m| m.get(&fd)) {
            Some(l) => l,
            None => return Err(err(10038)),
        };
        loop {
            match l.accept() {
                Ok((s, peer)) => {
                    s.set_nonblocking(true).map_err(io)?;
                    let _ = s.set_nodelay(true);
                    // the connection stays owned by the table (dropping it closes the socket); its id is the
                    // SOCKET value, which is what WSAPoll below needs
                    let cfd = s.as_raw_socket() as i64;
                    CONNS.lock().unwrap().get_or_insert_with(HashMap::new).insert(cfd, s);
                    return Ok(Some((cfd, peer.to_string())));
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(None),
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                // a connection aborted before it was accepted: try the next (as the Unix glue retries)
                Err(e) if matches!(e.raw_os_error(), Some(10052) | Some(10053) | Some(10054)) => continue,
                Err(e) => return Err(io(e)),
            }
        }
    }

    fn op_read(fd: i64, n: usize) -> Result<Option<Vec<u8>>, NetErr> {
        let mut b = vec![0u8; n];
        loop {
            let g = CONNS.lock().unwrap();
            let s = match g.as_ref().and_then(|m| m.get(&fd)) {
                Some(s) => s,
                None => return Err(err(10038)),
            };
            // Read on a &TcpStream: the table keeps the stream open across calls
            let mut r = s;
            match r.read(&mut b) {
                Ok(k) => {
                    b.truncate(k);
                    return Ok(Some(b));
                }
                // the client closed its end: an empty read, as Unix recv gives at EOF
                Err(e) if e.kind() == ErrorKind::BrokenPipe => {
                    b.truncate(0);
                    return Ok(Some(b));
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(None),
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(io(e)),
            }
        }
    }

    fn op_write(fd: i64, b: &[u8]) -> Result<i64, NetErr> {
        loop {
            let g = CONNS.lock().unwrap();
            let s = match g.as_ref().and_then(|m| m.get(&fd)) {
                Some(s) => s,
                None => return Err(err(10038)),
            };
            let mut w = s;
            match w.write(b) {
                Ok(k) => return Ok(k as i64),
                Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(0),
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(io(e)),
            }
        }
    }

    fn op_close(fd: i64) {
        // a listener first (a listener is never a connection), then a connection: dropping either closes it
        if LISTENERS.lock().unwrap().as_mut().and_then(|m| m.remove(&fd)).is_some() {
            return;
        }
        let c = CONNS.lock().unwrap().as_mut().and_then(|m| m.remove(&fd));
        drop(c);
    }

    fn op_poll(fds: &[(i64, i64)], timeout: f64) -> Result<Vec<i64>, NetErr> {
        let mut p: Vec<PollFd> = fds
            .iter()
            .map(|&(fd, ev)| PollFd {
                fd: fd as usize,
                events: (if ev & 1 != 0 { POLLRDNORM } else { 0 }) | (if ev & 4 != 0 { POLLWRNORM } else { 0 }),
                revents: 0,
            })
            .collect();
        if p.is_empty() {
            return Ok(vec![]);
        }
        let ms = if timeout < 0.0 { -1 } else { (timeout * 1000.0).ceil() as i32 };
        loop {
            let r = unsafe { WSAPoll(p.as_mut_ptr(), p.len() as u32, ms) };
            if r < 0 {
                let e = unsafe { WSAGetLastError() };
                if e == EINTR {
                    continue;
                }
                return Err(err(if e == 0 { ENOBUF } else { e }));
            }
            // back to Unix poll's numbers: 1 readable, 4 writable, otherwise nonzero when the socket errored
            // or hangs up (the event loop reads it and sees the error)
            return Ok(p
                .iter()
                .map(|x| {
                    let mut rev = 0i64;
                    if x.revents & (POLLRDNORM | POLLRDBAND) != 0 {
                        rev |= 1;
                    }
                    if x.revents & POLLWRNORM != 0 {
                        rev |= 4;
                    }
                    if rev == 0 && x.revents & (POLLERR | POLLHUP | POLLNVAL) != 0 {
                        rev = 8;
                    }
                    rev
                })
                .collect());
        }
    }

    fn op_random(n: usize) -> Result<Vec<u8>, NetErr> {
        // BCryptGenRandom with the system RNG: no file to open, so no file grant needed
        let mut b = vec![0u8; n];
        for c in b.chunks_mut(256) {
            if unsafe { BCryptGenRandom(std::ptr::null_mut(), c.as_mut_ptr(), c.len() as u32, SYSTEM_RNG) } != 0 {
                return Err(NetErr { kind: "OSError", msg: "[Errno 5] the system random source failed".into() });
            }
        }
        Ok(b)
    }

    pub fn ops() -> SrvOps {
        SrvOps { listen: op_listen, accept: op_accept, read: op_read, write: op_write, close: op_close, poll: op_poll, random: op_random }
    }
}
