
// ---------- the network, for Brevi's requests and httpx (pasted into the program; see rt_net.rs) ----------
// Sockets are the system's (connect, poll), names resolve with getaddrinfo, and TLS is the system's OpenSSL,
// loaded when a program first makes an HTTPS connection, as CPython's ssl module uses it. Errors carry
// CPython's messages ("[Errno 111] Connection refused" on Unix, "[WinError 10061] ..." on Windows; "timed out").
// Under --sandbox the program reaches only the hosts the caller granted (NET_ANY / NET_HOSTS, set by the
// compiler) and reads no environment variables.
// Unix: file descriptors, connect/fcntl/poll/close and dlopen'd libssl. Windows: Winsock SOCKET handles
// (ioctlsocket for non-blocking, select for a connecting socket, WSAPoll for readiness, closesocket) and the
// OpenSSL DLLs (libssl-1_1.dll / libssl-3-x64.dll with libcrypto-1_1.dll / libcrypto-3-x64.dll, which
// Python's DLLs folder holds; BREVI_SSL_LIB / BREVI_SSL_DIR name another copy). OpenSSL on Windows has no
// path to the system certificate store, so verification falls back to a CA bundle (SSL_CERT_FILE,
// REQUESTS_CA_BUNDLE or certifi next to a Python on PATH).
#[allow(non_snake_case, non_camel_case_types, dead_code)]
mod brevi_netglue {
    use super::*;
    use brevi_dyn::{NetErr, NetOps};
    use std::ffi::{c_char, c_int, c_long, c_ulong, c_void, CStr, CString};
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
    use std::sync::Mutex;
    use std::collections::HashMap;
    use std::time::Duration;

    type P = *mut c_void;

    #[cfg(unix)]
    extern "C" {
        fn socket(domain: c_int, ty: c_int, proto: c_int) -> c_int;
        fn connect(fd: c_int, addr: *const u8, len: u32) -> c_int;
        fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
        fn getsockopt(fd: c_int, level: c_int, name: c_int, val: *mut c_void, len: *mut u32) -> c_int;
        fn setsockopt(fd: c_int, level: c_int, name: c_int, val: *const c_void, len: u32) -> c_int;
        fn poll(fds: *mut PollFd, n: u64, timeout: c_int) -> c_int;
        fn close(fd: c_int) -> c_int;
        fn strerror(e: c_int) -> *const c_char;
        fn dlopen(f: *const c_char, flag: c_int) -> P;
        fn dlsym(h: P, s: *const c_char) -> P;
    }
    #[cfg(windows)]
    unsafe extern "system" {
        fn GetProcAddress(h: P, s: *const c_char) -> *mut c_void;
    }
    #[cfg(windows)]
    #[link(name = "ws2_32")]
    unsafe extern "system" {
        fn WSAStartup(version: u16, data: *mut u8) -> c_int;
        fn WSAGetLastError() -> c_int;
        fn socket(af: c_int, ty: c_int, proto: c_int) -> usize;
        fn connect(s: usize, name: *const u8, namelen: c_int) -> c_int;
        fn ioctlsocket(s: usize, cmd: c_long, argp: *mut u32) -> c_int;
        fn getsockopt(s: usize, level: c_int, name: c_int, val: *mut c_char, len: *mut c_int) -> c_int;
        fn setsockopt(s: usize, level: c_int, name: c_int, val: *const c_char, len: c_int) -> c_int;
        fn WSAPoll(fds: *mut PollFd, n: c_ulong, timeout: c_int) -> c_int;
        fn closesocket(s: usize) -> c_int;
        fn select(nfds: c_int, r: *mut FdSet, w: *mut FdSet, e: *mut FdSet, t: *const Timeval) -> c_int;
    }

    #[cfg(unix)]
    use std::os::fd::{AsRawFd, FromRawFd};
    #[cfg(windows)]
    use std::os::windows::io::{AsRawSocket, FromRawSocket};

    // poll's struct: fd, events, revents (winsock's WSAPOLLFD has the same fields with a SOCKET fd)
    #[cfg(unix)]
    #[repr(C)]
    struct PollFd {
        fd: c_int,
        events: i16,
        revents: i16,
    }
    #[cfg(windows)]
    #[repr(C)]
    struct PollFd {
        fd: usize,
        events: i16,
        revents: i16,
    }
    #[cfg(windows)]
    #[repr(C)]
    struct FdSet {
        count: u32,
        fds: [usize; 64],
    }
    #[cfg(windows)]
    #[repr(C)]
    struct Timeval {
        sec: c_long,
        usec: c_long,
    }
    #[cfg(windows)]
    #[repr(C, align(8))]
    struct WsaData([u8; 512]);

    #[cfg(target_os = "linux")]
    mod k {
        pub const AF_INET6: u16 = 10;
        pub const SOL_SOCKET: i32 = 1;
        pub const SO_ERROR: i32 = 4;
        pub const O_NONBLOCK: i32 = 0o4000;
        pub const EINPROGRESS: i32 = 115;
        pub const SOCK_CLOEXEC: i32 = 0o2000000;
        pub const SIN_LEN: bool = false;
        pub const EINTR: i32 = 4;
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    mod k {
        pub const AF_INET6: u16 = 30;
        pub const SOL_SOCKET: i32 = 0xffff;
        pub const SO_ERROR: i32 = 0x1007;
        pub const O_NONBLOCK: i32 = 4;
        pub const EINPROGRESS: i32 = 36;
        pub const SOCK_CLOEXEC: i32 = 0;
        pub const SIN_LEN: bool = true;
        pub const EINTR: i32 = 4;
    }
    #[cfg(windows)]
    mod k {
        pub const AF_INET6: u16 = 23;
        pub const SOL_SOCKET: i32 = 0xffff;
        pub const SO_ERROR: i32 = 0x1007;
        pub const SIN_LEN: bool = false;
        pub const INVALID_SOCKET: usize = usize::MAX;
        pub const FIONBIO: u32 = 0x8004_667e;
        pub const WSAEWOULDBLOCK: i32 = 10035;
        pub const WSAEINPROGRESS: i32 = 10036;
        pub const WSAEINTR: i32 = 10004;
        // winsock2.h's poll event masks: the program's events are poll's 1 = read, 4 = write
        pub const POLLRD: i16 = 0x0300; // POLLRDNORM | POLLRDBAND
        pub const POLLWR: i16 = 0x0010; // POLLWRNORM
        pub const POLLERR: i16 = 0x0001;
        pub const POLLHUP: i16 = 0x0002;
        pub const POLLNVAL: i16 = 0x0004;
    }
    const IPPROTO_TCP: i32 = 6;
    const TCP_NODELAY: i32 = 1;

    struct Conn {
        // None while a non-blocking connect is in progress
        s: Option<TcpStream>,
        pending: i64,
        addrs: Vec<SocketAddr>,
        // the error of the last address that failed, raised when none is left
        last_err: Option<(&'static str, String)>,
        ssl: P,
        tls_done: bool,
        blocking: bool,
        // seconds; < 0: none
        timeout: f64,
    }
    unsafe impl Send for Conn {}

    static CONNS: Mutex<Option<HashMap<i64, Conn>>> = Mutex::new(None);
    static NEXT: Mutex<i64> = Mutex::new(0);

    fn with<T>(id: i64, f: impl FnOnce(&mut Conn) -> Result<T, NetErr>) -> Result<T, NetErr> {
        let mut g = CONNS.lock().unwrap();
        let c = g.get_or_insert_with(HashMap::new).get_mut(&id).ok_or(NetErr { kind: "OSError", msg: "[Errno 9] Bad file descriptor".into() })?;
        f(c)
    }

    fn add(c: Conn) -> i64 {
        let mut n = NEXT.lock().unwrap();
        *n += 1;
        CONNS.lock().unwrap().get_or_insert_with(HashMap::new).insert(*n, c);
        *n
    }

    #[cfg(unix)]
    fn last_errno() -> i32 {
        std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
    }
    #[cfg(windows)]
    fn last_errno() -> i32 {
        unsafe { WSAGetLastError() }
    }

    #[cfg(unix)]
    fn errno_err(code: i32) -> NetErr {
        let msg = unsafe { CStr::from_ptr(strerror(code)) }.to_string_lossy().to_string();
        let kind = match std::io::Error::from_raw_os_error(code).kind() {
            std::io::ErrorKind::ConnectionRefused => "ConnectionRefusedError",
            std::io::ErrorKind::ConnectionReset => "ConnectionResetError",
            std::io::ErrorKind::ConnectionAborted => "ConnectionAbortedError",
            std::io::ErrorKind::BrokenPipe => "BrokenPipeError",
            std::io::ErrorKind::TimedOut => "TimeoutError",
            _ => "OSError",
        };
        NetErr { kind, msg: format!("[Errno {code}] {msg}") }
    }
    #[cfg(windows)]
    fn errno_err(code: i32) -> NetErr {
        // CPython's Windows wording: [WinError N] and the system's text for the socket error
        let (kind, text) = match code {
            10061 => ("ConnectionRefusedError", "No connection could be made because the target machine actively refused it"),
            10054 => ("ConnectionResetError", "An existing connection was forcibly closed by the remote host"),
            10053 => ("ConnectionAbortedError", "An established connection was aborted by the software in your host machine"),
            10060 => ("TimeoutError", "A connection attempt failed because the connected party did not properly respond after a period of time, or established connection failed because connected host has failed to respond"),
            10058 => ("BrokenPipeError", "A request to send or receive data was disallowed because the socket had already been shut down in that direction with a previous shutdown call"),
            10004 => ("InterruptedError", "A blocking operation was interrupted by a call to WSACancelBlockingCall"),
            10065 => ("OSError", "No route to host"),
            10051 => ("OSError", "Network is unreachable"),
            10050 => ("OSError", "Network is down"),
            10048 => ("OSError", "Only one usage of each socket address (protocol/network address/port) is normally permitted"),
            10049 => ("OSError", "The requested address is not valid in its context"),
            10057 => ("OSError", "A request to send or receive data was disallowed because the socket is not connected and (when sending on a datagram socket using a sendto call) no address was supplied"),
            10038 => ("OSError", "An operation was attempted on something that is not a socket"),
            _ => ("OSError", ""),
        };
        let msg = if text.is_empty() { std::io::Error::from_raw_os_error(code).to_string() } else { text.to_string() };
        NetErr { kind, msg: format!("[WinError {code}] {msg}") }
    }

    fn io_err(e: std::io::Error, tls: bool) -> NetErr {
        if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) {
            return timed_out(tls, "read");
        }
        match e.raw_os_error() {
            Some(c) => errno_err(c),
            None => NetErr { kind: "OSError", msg: e.to_string() },
        }
    }

    fn timed_out(tls: bool, op: &str) -> NetErr {
        NetErr { kind: "TimeoutError", msg: if tls { format!("The {op} operation timed out") } else { "timed out".into() } }
    }

    // getaddrinfo's errors, as socket.gaierror words them
    #[cfg(unix)]
    fn gaierror(e: std::io::Error, _host: &str) -> NetErr {
        let m = e.to_string();
        let m = m.strip_prefix("failed to lookup address information: ").unwrap_or(&m).to_string();
        let code = match m.as_str() {
            "Name or service not known" | "nodename nor servname provided, or not known" => -2,
            "Temporary failure in name resolution" => -3,
            "Non-recoverable failure in name resolution" => -4,
            "No address associated with hostname" => -5,
            "Servname not supported for ai_socktype" => -8,
            _ => -2,
        };
        let code = if cfg!(target_os = "linux") { code } else { 8 };
        NetErr { kind: "socket.gaierror", msg: format!("[Errno {code}] {m}") }
    }
    #[cfg(windows)]
    fn gaierror(e: std::io::Error, _host: &str) -> NetErr {
        // getaddrinfo's WSA code (11001 WSAHOST_NOT_FOUND, 11002 WSATRY_AGAIN, ...), as CPython reports it
        let code = e.raw_os_error().unwrap_or(11001);
        NetErr { kind: "socket.gaierror", msg: format!("[Errno {code}] getaddrinfo failed") }
    }

    fn resolve(host: &str, port: u16) -> Result<Vec<SocketAddr>, NetErr> {
        let h = host.trim_start_matches('[').trim_end_matches(']');
        match (h, port).to_socket_addrs() {
            Ok(a) => Ok(a.collect()),
            Err(e) => Err(gaierror(e, h)),
        }
    }

    fn sockaddr(a: &SocketAddr) -> Vec<u8> {
        let mut b = vec![];
        let fam = |b: &mut Vec<u8>, f: u16, len: u8| {
            if k::SIN_LEN {
                b.push(len);
                b.push(f as u8);
            } else {
                b.extend_from_slice(&f.to_ne_bytes());
            }
        };
        match a {
            SocketAddr::V4(v) => {
                fam(&mut b, 2, 16);
                b.extend_from_slice(&v.port().to_be_bytes());
                b.extend_from_slice(&v.ip().octets());
                b.extend_from_slice(&[0; 8]);
            }
            SocketAddr::V6(v) => {
                fam(&mut b, k::AF_INET6, 28);
                b.extend_from_slice(&v.port().to_be_bytes());
                b.extend_from_slice(&v.flowinfo().to_ne_bytes());
                b.extend_from_slice(&v.ip().octets());
                b.extend_from_slice(&v.scope_id().to_ne_bytes());
            }
        }
        b
    }

    #[cfg(windows)]
    fn wsa_init() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| unsafe {
            let mut d = WsaData([0u8; 512]);
            WSAStartup(0x0202, d.0.as_mut_ptr());
        });
    }

    // Starts a non-blocking connect: the socket, or the error.
    #[cfg(unix)]
    fn start(a: &SocketAddr) -> Result<i64, NetErr> {
        let dom = if a.is_ipv4() { 2 } else { k::AF_INET6 as i32 };
        let fd = unsafe { socket(dom, 1 | k::SOCK_CLOEXEC, 0) };
        if fd < 0 {
            return Err(errno_err(last_errno()));
        }
        unsafe {
            let fl = fcntl(fd, 3);
            fcntl(fd, 4, fl | k::O_NONBLOCK);
            let one: c_int = 1;
            setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &one as *const c_int as *const c_void, 4);
        }
        let sa = sockaddr(a);
        let r = unsafe { connect(fd, sa.as_ptr(), sa.len() as u32) };
        if r != 0 {
            let e = last_errno();
            if e != k::EINPROGRESS {
                unsafe { close(fd) };
                return Err(errno_err(e));
            }
        }
        Ok(fd as i64)
    }
    #[cfg(windows)]
    fn start(a: &SocketAddr) -> Result<i64, NetErr> {
        wsa_init();
        let dom = if a.is_ipv4() { 2 } else { k::AF_INET6 as c_int };
        let s = unsafe { socket(dom, 1, 0) };
        if s == k::INVALID_SOCKET {
            return Err(errno_err(last_errno()));
        }
        unsafe {
            let mut nonblocking: u32 = 1;
            ioctlsocket(s, k::FIONBIO as c_long, &mut nonblocking);
            let one: c_int = 1;
            setsockopt(s, IPPROTO_TCP, TCP_NODELAY, &one as *const c_int as *const c_char, 4);
        }
        let sa = sockaddr(a);
        let r = unsafe { connect(s, sa.as_ptr(), sa.len() as c_int) };
        if r != 0 {
            let e = last_errno();
            if e != k::WSAEWOULDBLOCK && e != k::WSAEINPROGRESS {
                unsafe { closesocket(s) };
                return Err(errno_err(e));
            }
        }
        Ok(s as i64)
    }

    #[cfg(unix)]
    fn close_sock(h: i64) {
        unsafe { close(h as c_int) };
    }
    #[cfg(windows)]
    fn close_sock(h: i64) {
        unsafe { closesocket(h as usize) };
    }

    #[cfg(unix)]
    fn to_stream(h: i64) -> TcpStream {
        unsafe { TcpStream::from_raw_fd(h as c_int) }
    }
    #[cfg(windows)]
    fn to_stream(h: i64) -> TcpStream {
        unsafe { TcpStream::from_raw_socket(h as std::os::windows::io::RawSocket) }
    }
    #[cfg(unix)]
    fn raw_of(s: &TcpStream) -> i64 {
        s.as_raw_fd() as i64
    }
    #[cfg(windows)]
    fn raw_of(s: &TcpStream) -> i64 {
        s.as_raw_socket() as i64
    }
    // OpenSSL's SSL_set_fd takes an int (as CPython's _ssl passes its SOCKET)
    #[cfg(unix)]
    fn ssl_fd(s: &TcpStream) -> c_int {
        s.as_raw_fd()
    }
    #[cfg(windows)]
    fn ssl_fd(s: &TcpStream) -> c_int {
        raw_of(s) as c_int
    }

    #[cfg(unix)]
    fn so_error(fd: i64) -> i32 {
        let mut e: c_int = 0;
        let mut l: u32 = 4;
        unsafe { getsockopt(fd as c_int, k::SOL_SOCKET, k::SO_ERROR, &mut e as *mut c_int as *mut c_void, &mut l) };
        e
    }
    #[cfg(windows)]
    fn so_error(h: i64) -> i32 {
        let mut e: c_int = 0;
        let mut l: c_int = 4;
        unsafe { getsockopt(h as usize, k::SOL_SOCKET, k::SO_ERROR, &mut e as *mut c_int as *mut c_char, &mut l) };
        e
    }

    #[cfg(unix)]
    fn wait_fd(fd: i64, events: i16, timeout: f64) -> Result<bool, NetErr> {
        let mut p = PollFd { fd: fd as c_int, events, revents: 0 };
        let ms = if timeout < 0.0 { -1 } else { (timeout * 1000.0).ceil() as c_int };
        loop {
            let r = unsafe { poll(&mut p, 1, ms) };
            if r < 0 {
                let e = last_errno();
                if e == k::EINTR {
                    continue;
                }
                return Err(errno_err(e));
            }
            return Ok(r > 0);
        }
    }
    // A connecting socket, with select rather than WSAPoll: WSAPoll before Windows 10 1809 did not report a
    // failed connect. A failed connect shows in the write set or the exception set; the caller reads SO_ERROR.
    #[cfg(windows)]
    fn wait_fd(h: i64, events: i16, timeout: f64) -> Result<bool, NetErr> {
        let mut read = FdSet { count: 0, fds: [0; 64] };
        let mut write = FdSet { count: 0, fds: [0; 64] };
        let mut fail = FdSet { count: 0, fds: [0; 64] };
        if events & 1 != 0 {
            read.fds[0] = h as usize;
            read.count = 1;
        }
        if events & 4 != 0 {
            write.fds[0] = h as usize;
            write.count = 1;
        }
        fail.fds[0] = h as usize;
        fail.count = 1;
        let mut tv = Timeval { sec: 0, usec: 0 };
        let tp: *const Timeval = if timeout < 0.0 {
            std::ptr::null()
        } else {
            let us = (timeout * 1_000_000.0).ceil() as i64;
            tv.sec = (us / 1_000_000) as c_long;
            tv.usec = (us % 1_000_000) as c_long;
            &tv as *const Timeval
        };
        let r = unsafe { select(0, &mut read, &mut write, &mut fail, tp) };
        if r < 0 {
            return Err(errno_err(last_errno()));
        }
        Ok(read.count > 0 || write.count > 0 || fail.count > 0)
    }

    fn finish(fd: i64, timeout: f64) -> Conn {
        let s = to_stream(fd);
        s.set_nonblocking(false).ok();
        let mut c = Conn { s: Some(s), pending: -1, addrs: vec![], last_err: None, ssl: std::ptr::null_mut(), tls_done: false, blocking: true, timeout: -1.0 };
        set_timeout(&mut c, timeout);
        c
    }

    fn set_timeout(c: &mut Conn, t: f64) {
        c.timeout = t;
        if let Some(s) = &c.s {
            let d = if t < 0.0 { None } else { Some(Duration::from_secs_f64(t.max(1e-6))) };
            s.set_read_timeout(d).ok();
            s.set_write_timeout(d).ok();
        }
    }

    fn host_ok(host: &str) -> bool {
        NET_ANY || NET_HOSTS.iter().any(|h| h.eq_ignore_ascii_case(host.trim_start_matches('[').trim_end_matches(']')))
    }

    fn denied(host: &str) -> NetErr {
        NetErr { kind: "PermissionError", msg: format!("brevi sandbox: the caller did not grant the network to {host} (--allow-net {host}, or --allow net for every host)") }
    }

    fn op_connect(host: &str, port: u16, timeout: f64) -> Result<i64, NetErr> {
        if !host_ok(host) {
            return Err(denied(host));
        }
        let addrs = resolve(host, port)?;
        let mut last = None;
        for a in &addrs {
            match start(a) {
                Ok(fd) => {
                    if !wait_fd(fd, 4, timeout)? {
                        close_sock(fd);
                        last = Some(timed_out(false, "connect"));
                        continue;
                    }
                    let e = so_error(fd);
                    if e != 0 {
                        close_sock(fd);
                        last = Some(errno_err(e));
                        continue;
                    }
                    return Ok(add(finish(fd, timeout)));
                }
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap_or(NetErr { kind: "OSError", msg: "getaddrinfo returns an empty list".into() }))
    }

    fn op_connect_nb(host: &str, port: u16) -> Result<i64, NetErr> {
        if !host_ok(host) {
            return Err(denied(host));
        }
        let addrs = resolve(host, port)?;
        let c = Conn { s: None, pending: -1, addrs, last_err: None, ssl: std::ptr::null_mut(), tls_done: false, blocking: false, timeout: -1.0 };
        let id = add(c);
        with(id, |c| next_addr(c))?;
        Ok(id)
    }

    // Starts the connect to the next address; Err when none is left.
    fn next_addr(c: &mut Conn) -> Result<(), NetErr> {
        while !c.addrs.is_empty() {
            let a = c.addrs.remove(0);
            match start(&a) {
                Ok(fd) => {
                    c.pending = fd;
                    return Ok(());
                }
                Err(e) => c.last_err = Some((e.kind, e.msg)),
            }
        }
        let (kind, msg) = c.last_err.take().unwrap_or(("OSError", "getaddrinfo returns an empty list".into()));
        Err(NetErr { kind, msg })
    }

    fn op_ready(id: i64) -> Result<bool, NetErr> {
        with(id, |c| {
            if c.s.is_some() {
                return Ok(true);
            }
            if !wait_fd(c.pending, 4, 0.0)? {
                return Ok(false);
            }
            let e = so_error(c.pending);
            if e == 0 {
                let s = to_stream(c.pending);
                s.set_nonblocking(true).ok();
                c.s = Some(s);
                c.pending = -1;
                return Ok(true);
            }
            close_sock(c.pending);
            c.pending = -1;
            let er = errno_err(e);
            c.last_err = Some((er.kind, er.msg));
            next_addr(c)?;
            Ok(false)
        })
    }

    // ---------- OpenSSL ----------
    struct Ssl {
        TLS_client_method: unsafe extern "C" fn() -> P,
        SSL_CTX_new: unsafe extern "C" fn(P) -> P,
        SSL_CTX_free: unsafe extern "C" fn(P),
        SSL_CTX_set_verify: unsafe extern "C" fn(P, c_int, P),
        SSL_CTX_set_default_verify_paths: unsafe extern "C" fn(P) -> c_int,
        SSL_CTX_load_verify_locations: unsafe extern "C" fn(P, *const c_char, *const c_char) -> c_int,
        SSL_CTX_ctrl: unsafe extern "C" fn(P, c_int, c_long, P) -> c_long,
        SSL_CTX_set_alpn_protos: unsafe extern "C" fn(P, *const u8, u32) -> c_int,
        SSL_new: unsafe extern "C" fn(P) -> P,
        SSL_free: unsafe extern "C" fn(P),
        SSL_set_fd: unsafe extern "C" fn(P, c_int) -> c_int,
        SSL_ctrl: unsafe extern "C" fn(P, c_int, c_long, P) -> c_long,
        SSL_set1_host: unsafe extern "C" fn(P, *const c_char) -> c_int,
        SSL_get0_param: unsafe extern "C" fn(P) -> P,
        X509_VERIFY_PARAM_set1_ip_asc: unsafe extern "C" fn(P, *const c_char) -> c_int,
        SSL_connect: unsafe extern "C" fn(P) -> c_int,
        SSL_read: unsafe extern "C" fn(P, *mut u8, c_int) -> c_int,
        SSL_write: unsafe extern "C" fn(P, *const u8, c_int) -> c_int,
        SSL_get_error: unsafe extern "C" fn(P, c_int) -> c_int,
        SSL_shutdown: unsafe extern "C" fn(P) -> c_int,
        SSL_get_verify_result: unsafe extern "C" fn(P) -> c_long,
        X509_verify_cert_error_string: unsafe extern "C" fn(c_long) -> *const c_char,
        ERR_get_error: unsafe extern "C" fn() -> u64,
        ERR_clear_error: unsafe extern "C" fn(),
        ERR_reason_error_string: unsafe extern "C" fn(u64) -> *const c_char,
    }
    unsafe impl Send for Ssl {}
    unsafe impl Sync for Ssl {}

    static SSL: std::sync::OnceLock<Option<Ssl>> = std::sync::OnceLock::new();

    fn ssl() -> Result<&'static Ssl, NetErr> {
        SSL.get_or_init(|| unsafe { load_ssl() }).as_ref().ok_or_else(ssl_missing)
    }

    fn ssl_missing() -> NetErr {
        #[cfg(unix)]
        let msg = "brevi: HTTPS needs the system's OpenSSL (libssl.so.3 or libssl.so.1.1), which was not found".to_string();
        #[cfg(windows)]
        let msg = "brevi: HTTPS needs the OpenSSL DLLs (libssl-1_1.dll or libssl-3-x64.dll, with libcrypto-1_1.dll / libcrypto-3-x64.dll; Python's DLLs folder has them, or set BREVI_SSL_LIB / BREVI_SSL_DIR)".to_string();
        NetErr { kind: "ssl.SSLError", msg }
    }

    #[cfg(unix)]
    unsafe fn load_ssl() -> Option<Ssl> {
        let names: &[&str] = if cfg!(target_os = "macos") {
            &["/opt/homebrew/opt/openssl@3/lib/libssl.3.dylib", "/usr/local/opt/openssl@3/lib/libssl.3.dylib", "/opt/local/lib/libssl.3.dylib"]
        } else {
            &["libssl.so.3", "libssl.so.1.1", "libssl.so"]
        };
        let mut h = std::ptr::null_mut();
        for n in names {
            let c = CString::new(*n).unwrap();
            // RTLD_NOW | RTLD_GLOBAL, so libcrypto's symbols are found through libssl
            h = dlopen(c.as_ptr(), 2 | if cfg!(target_os = "macos") { 8 } else { 0x100 });
            if !h.is_null() {
                break;
            }
        }
        if h.is_null() {
            return None;
        }
        macro_rules! sym {
            ($n:literal) => {{
                let c = CString::new($n).unwrap();
                let p = dlsym(h, c.as_ptr());
                if p.is_null() {
                    return None;
                }
                std::mem::transmute(p)
            }};
        }
        Some(Ssl {
            TLS_client_method: sym!("TLS_client_method"),
            SSL_CTX_new: sym!("SSL_CTX_new"),
            SSL_CTX_free: sym!("SSL_CTX_free"),
            SSL_CTX_set_verify: sym!("SSL_CTX_set_verify"),
            SSL_CTX_set_default_verify_paths: sym!("SSL_CTX_set_default_verify_paths"),
            SSL_CTX_load_verify_locations: sym!("SSL_CTX_load_verify_locations"),
            SSL_CTX_ctrl: sym!("SSL_CTX_ctrl"),
            SSL_CTX_set_alpn_protos: sym!("SSL_CTX_set_alpn_protos"),
            SSL_new: sym!("SSL_new"),
            SSL_free: sym!("SSL_free"),
            SSL_set_fd: sym!("SSL_set_fd"),
            SSL_ctrl: sym!("SSL_ctrl"),
            SSL_set1_host: sym!("SSL_set1_host"),
            SSL_get0_param: sym!("SSL_get0_param"),
            X509_VERIFY_PARAM_set1_ip_asc: sym!("X509_VERIFY_PARAM_set1_ip_asc"),
            SSL_connect: sym!("SSL_connect"),
            SSL_read: sym!("SSL_read"),
            SSL_write: sym!("SSL_write"),
            SSL_get_error: sym!("SSL_get_error"),
            SSL_shutdown: sym!("SSL_shutdown"),
            SSL_get_verify_result: sym!("SSL_get_verify_result"),
            X509_verify_cert_error_string: sym!("X509_verify_cert_error_string"),
            ERR_get_error: sym!("ERR_get_error"),
            ERR_clear_error: sym!("ERR_clear_error"),
            ERR_reason_error_string: sym!("ERR_reason_error_string"),
        })
    }

    // The libssl and libcrypto DLLs, crypto loaded first so libssl's import of it resolves. Only absolute
    // paths are tried: BREVI_SSL_LIB (a libssl file), BREVI_SSL_DIR, and the Python DLL folder embedded at
    // build time (BREVI_SSL_DIR const). No program-folder, CWD, PATH or bare-name candidate: a planted
    // libssl must never run.
    #[cfg(windows)]
    unsafe fn ssl_handles() -> Option<(P, P)> {
        // The caller's own copy (BREVI_SSL_LIB, BREVI_SSL_DIR; never read under the sandbox): not pinned, but
        // loaded only by an absolute path with the safe dependency search (libpin.rs open_unpinned).
        if !NET_SANDBOX {
            let mut files: Vec<String> = vec![];
            if let Ok(p) = std::env::var("BREVI_SSL_LIB") {
                files.push(p);
            }
            if let Ok(d) = std::env::var("BREVI_SSL_DIR") {
                for n in ["libssl-3-x64.dll", "libssl-3.dll", "libssl-1_1-x64.dll", "libssl-1_1.dll"] {
                    files.push(format!("{d}\\{n}"));
                }
            }
            for f in &files {
                let Ok(crypto) = brevi_libpin::open_unpinned(&f.replace("libssl", "libcrypto")) else { continue };
                let Ok(lib) = brevi_libpin::open_unpinned(f) else { continue };
                return Some((lib, crypto));
            }
        }
        // Python's own copy, found when the program was built, each DLL pinned by SHA-256 (SSL_PINS).
        let pin = |f: &str| SSL_PINS.iter().find(|(p, _)| *p == f).map(|(_, s)| *s);
        for (f, _) in SSL_PINS.iter().filter(|(p, _)| p.contains("libssl")) {
            let cf = f.replace("libssl", "libcrypto");
            let Some(csum) = pin(&cf) else { continue };
            let Ok(crypto) = brevi_libpin::open(&cf, csum, 0) else { continue };
            let Ok(lib) = brevi_libpin::open(f, pin(f).unwrap_or(""), 0) else { continue };
            return Some((lib, crypto));
        }
        None
    }

    #[cfg(windows)]
    unsafe fn load_ssl() -> Option<Ssl> {
        let (lib, crypto) = ssl_handles()?;
        macro_rules! sym {
            ($n:literal) => {{
                let c = CString::new($n).unwrap();
                let mut p = GetProcAddress(lib, c.as_ptr());
                if p.is_null() {
                    p = GetProcAddress(crypto, c.as_ptr());
                }
                if p.is_null() {
                    return None;
                }
                std::mem::transmute(p)
            }};
        }
        Some(Ssl {
            TLS_client_method: sym!("TLS_client_method"),
            SSL_CTX_new: sym!("SSL_CTX_new"),
            SSL_CTX_free: sym!("SSL_CTX_free"),
            SSL_CTX_set_verify: sym!("SSL_CTX_set_verify"),
            SSL_CTX_set_default_verify_paths: sym!("SSL_CTX_set_default_verify_paths"),
            SSL_CTX_load_verify_locations: sym!("SSL_CTX_load_verify_locations"),
            SSL_CTX_ctrl: sym!("SSL_CTX_ctrl"),
            SSL_CTX_set_alpn_protos: sym!("SSL_CTX_set_alpn_protos"),
            SSL_new: sym!("SSL_new"),
            SSL_free: sym!("SSL_free"),
            SSL_set_fd: sym!("SSL_set_fd"),
            SSL_ctrl: sym!("SSL_ctrl"),
            SSL_set1_host: sym!("SSL_set1_host"),
            SSL_get0_param: sym!("SSL_get0_param"),
            X509_VERIFY_PARAM_set1_ip_asc: sym!("X509_VERIFY_PARAM_set1_ip_asc"),
            SSL_connect: sym!("SSL_connect"),
            SSL_read: sym!("SSL_read"),
            SSL_write: sym!("SSL_write"),
            SSL_get_error: sym!("SSL_get_error"),
            SSL_shutdown: sym!("SSL_shutdown"),
            SSL_get_verify_result: sym!("SSL_get_verify_result"),
            X509_verify_cert_error_string: sym!("X509_verify_cert_error_string"),
            ERR_get_error: sym!("ERR_get_error"),
            ERR_clear_error: sym!("ERR_clear_error"),
            ERR_reason_error_string: sym!("ERR_reason_error_string"),
        })
    }

    // A CA bundle for verification on Windows: the environment overrides (outside the sandbox), then the
    // certifi bundle next to the Python the program was built with (BREVI_CA_FILE). No PATH or fixed
    // C:\PythonXXX search. None: verification uses OpenSSL's own default paths.
    #[cfg(windows)]
    fn ca_bundle() -> Option<String> {
        if !NET_SANDBOX {
            for v in ["SSL_CERT_FILE", "REQUESTS_CA_BUNDLE", "CURL_CA_BUNDLE"] {
                if let Ok(p) = std::env::var(v) {
                    if std::path::Path::new(&p).is_file() {
                        return Some(p);
                    }
                }
            }
        }
        if !BREVI_CA_FILE.is_empty() && std::path::Path::new(BREVI_CA_FILE).is_file() {
            return Some(BREVI_CA_FILE.to_string());
        }
        None
    }

    fn ssl_reason(s: &Ssl) -> String {
        let e = unsafe { (s.ERR_get_error)() };
        unsafe { (s.ERR_clear_error)() };
        if e == 0 {
            return String::new();
        }
        let p = unsafe { (s.ERR_reason_error_string)(e) };
        if p.is_null() {
            return format!("error {e:x}");
        }
        unsafe { CStr::from_ptr(p) }.to_string_lossy().to_string()
    }

    fn op_tls(id: i64, sni: &str, verify: bool, cafile: &str, capath: &str, check_host: bool) -> Result<(), NetErr> {
        let s = ssl()?;
        // a directory given as the CA bundle is a directory of certificates (requests' verify=DIR)
        let (cafile, capath) = if !cafile.is_empty() && capath.is_empty() && std::path::Path::new(cafile).is_dir() { ("", cafile) } else { (cafile, capath) };
        if verify && NET_SANDBOX && !NET_READ && !(cafile.is_empty() && capath.is_empty()) {
            let p = if cafile.is_empty() { capath } else { cafile };
            return Err(NetErr { kind: "PermissionError", msg: format!("brevi sandbox: reading the CA certificates {p} needs --allow read") });
        }
        // held to folders (--allow-read DIR): the CA file or folder is inside them
        for p in [cafile, capath].into_iter().filter(|p| verify && NET_SANDBOX && !p.is_empty()) {
            brevi_fs_path(p, false).map_err(|msg| NetErr { kind: "PermissionError", msg })?;
        }
        with(id, |c| unsafe {
            let ctx = (s.SSL_CTX_new)((s.TLS_client_method)());
            if ctx.is_null() {
                return Err(NetErr { kind: "ssl.SSLError", msg: ssl_reason(s) });
            }
            // TLS 1.2 at least, as CPython's default context
            (s.SSL_CTX_ctrl)(ctx, 123, 0x0303, std::ptr::null_mut());
            (s.SSL_CTX_set_alpn_protos)(ctx, b"\x08http/1.1".as_ptr(), 9);
            if verify {
                (s.SSL_CTX_set_verify)(ctx, 1, std::ptr::null_mut());
                if cafile.is_empty() && capath.is_empty() {
                    (s.SSL_CTX_set_default_verify_paths)(ctx);
                    // Windows: OpenSSL's default paths hold no certificates; add a CA bundle when one is found
                    #[cfg(windows)]
                    {
                        if let Some(b) = ca_bundle() {
                            let f = CString::new(b).unwrap_or_default();
                            (s.SSL_CTX_load_verify_locations)(ctx, f.as_ptr(), std::ptr::null());
                        }
                    }
                } else {
                    let f = CString::new(cafile).unwrap_or_default();
                    let d = CString::new(capath).unwrap_or_default();
                    let ok = (s.SSL_CTX_load_verify_locations)(ctx, if cafile.is_empty() { std::ptr::null() } else { f.as_ptr() }, if capath.is_empty() { std::ptr::null() } else { d.as_ptr() });
                    if ok != 1 {
                        (s.ERR_clear_error)();
                        (s.SSL_CTX_free)(ctx);
                        return Err(NetErr { kind: "OSError", msg: format!("Could not find a suitable TLS CA certificate bundle, invalid path: {}", if cafile.is_empty() { capath } else { cafile }) });
                    }
                }
            } else {
                (s.SSL_CTX_set_verify)(ctx, 0, std::ptr::null_mut());
            }
            let ssl = (s.SSL_new)(ctx);
            (s.SSL_CTX_free)(ctx);
            let fd = c.s.as_ref().map_or(-1, ssl_fd);
            (s.SSL_set_fd)(ssl, fd);
            let name = CString::new(sni.trim_start_matches('[').trim_end_matches(']')).unwrap_or_default();
            let is_ip = sni.trim_start_matches('[').trim_end_matches(']').parse::<std::net::IpAddr>().is_ok();
            if !is_ip {
                (s.SSL_ctrl)(ssl, 55, 0, name.as_ptr() as P);
            }
            if verify && check_host {
                if is_ip {
                    (s.X509_VERIFY_PARAM_set1_ip_asc)((s.SSL_get0_param)(ssl), name.as_ptr());
                } else {
                    (s.SSL_set1_host)(ssl, name.as_ptr());
                }
            }
            c.ssl = ssl;
            Ok(())
        })
    }

    // 0 done, 1 wants read, 2 wants write
    fn op_handshake(id: i64) -> Result<i64, NetErr> {
        let s = ssl()?;
        with(id, |c| unsafe {
            loop {
                let r = (s.SSL_connect)(c.ssl);
                if r == 1 {
                    c.tls_done = true;
                    return Ok(0);
                }
                let e = (s.SSL_get_error)(c.ssl, r);
                let want = match e {
                    2 => 1,
                    3 => 2,
                    _ => 0,
                };
                if want != 0 {
                    if !c.blocking {
                        return Ok(want);
                    }
                    // a blocking socket stops waiting only at its timeout
                    return Err(timed_out(true, "handshake"));
                }
                let v = (s.SSL_get_verify_result)(c.ssl);
                if v != 0 {
                    (s.ERR_clear_error)();
                    let why = CStr::from_ptr((s.X509_verify_cert_error_string)(v)).to_string_lossy().to_string();
                    return Err(NetErr { kind: "ssl.SSLCertVerificationError", msg: format!("[SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: {why} (_ssl.c:1035)") });
                }
                let why = ssl_reason(s);
                if why.is_empty() {
                    return Err(NetErr { kind: "ssl.SSLError", msg: "[SSL: UNEXPECTED_EOF_WHILE_READING] EOF occurred in violation of protocol (_ssl.c:1035)".into() });
                }
                return Err(NetErr { kind: "ssl.SSLError", msg: format!("[SSL: {}] {} (_ssl.c:1035)", why.to_ascii_uppercase().replace(' ', "_"), why) });
            }
        })
    }

    fn op_send(id: i64, b: &[u8]) -> Result<i64, NetErr> {
        with(id, |c| {
            if !c.ssl.is_null() {
                let s = ssl()?;
                {
                    let r = unsafe { (s.SSL_write)(c.ssl, b.as_ptr(), b.len().min(1 << 30) as c_int) };
                    if r > 0 {
                        return Ok(r as i64);
                    }
                    let e = unsafe { (s.SSL_get_error)(c.ssl, r) };
                    if e == 2 || e == 3 {
                        if !c.blocking {
                            return Ok(if e == 2 { -1 } else { -2 });
                        }
                        return Err(timed_out(true, "write"));
                    }
                    let os = last_errno();
                    if e == 5 && os != 0 {
                        return Err(errno_err(os));
                    }
                    return Err(NetErr { kind: "ssl.SSLError", msg: ssl_reason(s) });
                }
            }
            let st = c.s.as_mut().ok_or(NetErr { kind: "OSError", msg: "[Errno 107] Transport endpoint is not connected".into() })?;
            match st.write(b) {
                Ok(k) => Ok(k as i64),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && !c.blocking => Ok(-2),
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => Err(timed_out(false, "write")),
                Err(e) => Err(io_err(e, false)),
            }
        })
    }

    fn op_recv(id: i64, n: usize) -> Result<Vec<u8>, NetErr> {
        with(id, |c| {
            let mut buf = vec![0u8; n];
            if !c.ssl.is_null() {
                let s = ssl()?;
                loop {
                    let r = unsafe { (s.SSL_read)(c.ssl, buf.as_mut_ptr(), n as c_int) };
                    if r > 0 {
                        buf.truncate(r as usize);
                        return Ok(buf);
                    }
                    let e = unsafe { (s.SSL_get_error)(c.ssl, r) };
                    match e {
                        // the peer closed the TLS session, or the connection without closing it
                        6 => return Ok(vec![]),
                        2 | 3 => {
                            if !c.blocking {
                                return Err(NetErr { kind: if e == 2 { "want_read" } else { "want_write" }, msg: String::new() });
                            }
                            return Err(timed_out(true, "read"));
                        }
                        5 => {
                            let os = last_errno();
                            unsafe { (s.ERR_clear_error)() };
                            #[cfg(unix)]
                            let again = os == 11 || os == 35;
                            #[cfg(windows)]
                            let again = os == 10035;
                            if os != 0 && !again {
                                return Err(errno_err(os));
                            }
                            return Ok(vec![]);
                        }
                        _ => {
                            let why = ssl_reason(s);
                            if why.contains("unexpected eof") {
                                return Ok(vec![]);
                            }
                            return Err(NetErr { kind: "ssl.SSLError", msg: format!("[SSL] {why} (_ssl.c:2706)") });
                        }
                    }
                }
            }
            let st = c.s.as_mut().ok_or(NetErr { kind: "OSError", msg: "[Errno 107] Transport endpoint is not connected".into() })?;
            match st.read(&mut buf) {
                Ok(k) => {
                    buf.truncate(k);
                    Ok(buf)
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && !c.blocking => Err(NetErr { kind: "want_read", msg: String::new() }),
                Err(e) => Err(io_err(e, false)),
            }
        })
    }

    fn op_settimeout(id: i64, t: f64) {
        let _ = with(id, |c| {
            set_timeout(c, t);
            Ok(())
        });
    }

    fn op_setblocking(id: i64, b: bool) {
        let _ = with(id, |c| {
            c.blocking = b;
            if let Some(s) = &c.s {
                s.set_nonblocking(!b).ok();
            }
            Ok(())
        });
    }

    fn op_fileno(id: i64) -> i64 {
        with(id, |c| Ok(c.s.as_ref().map_or(c.pending, raw_of))).unwrap_or(-1)
    }

    fn op_close(id: i64) {
        let c = CONNS.lock().unwrap().get_or_insert_with(HashMap::new).remove(&id);
        if let Some(c) = c {
            if !c.ssl.is_null() {
                if let Ok(s) = ssl() {
                    unsafe {
                        if c.tls_done {
                            (s.SSL_shutdown)(c.ssl);
                        }
                        (s.SSL_free)(c.ssl);
                        (s.ERR_clear_error)();
                    }
                }
            }
            if c.pending >= 0 {
                close_sock(c.pending);
            }
        }
    }

    #[cfg(unix)]
    fn op_poll(fds: &[(i64, i64)], timeout: f64) -> Result<Vec<i64>, NetErr> {
        let mut p: Vec<PollFd> = fds.iter().map(|&(fd, ev)| PollFd { fd: fd as c_int, events: ev as i16, revents: 0 }).collect();
        let ms = if timeout < 0.0 { -1 } else { (timeout * 1000.0).ceil() as c_int };
        loop {
            let r = unsafe { poll(p.as_mut_ptr(), p.len() as u64, ms) };
            if r < 0 {
                let e = last_errno();
                if e == k::EINTR {
                    continue;
                }
                return Err(errno_err(e));
            }
            return Ok(p.iter().map(|x| x.revents as i64).collect());
        }
    }
    #[cfg(windows)]
    fn op_poll(fds: &[(i64, i64)], timeout: f64) -> Result<Vec<i64>, NetErr> {
        if fds.is_empty() {
            return Ok(vec![]);
        }
        let mut p: Vec<PollFd> = fds
            .iter()
            .map(|&(fd, ev)| {
                let mut events = 0i16;
                if ev & 1 != 0 {
                    events |= k::POLLRD;
                }
                if ev & 4 != 0 {
                    events |= k::POLLWR;
                }
                PollFd { fd: fd as usize, events, revents: 0 }
            })
            .collect();
        let ms = if timeout < 0.0 { -1 } else { (timeout * 1000.0).ceil().min(i32::MAX as f64) as c_int };
        loop {
            let r = unsafe { WSAPoll(p.as_mut_ptr(), p.len() as c_ulong, ms) };
            if r < 0 {
                let e = last_errno();
                if e == k::WSAEINTR {
                    continue;
                }
                return Err(errno_err(e));
            }
            return Ok(p
                .iter()
                .zip(fds)
                .map(|(x, &(_, want))| {
                    let mut ev = 0i64;
                    if x.revents & k::POLLRD != 0 {
                        ev |= 1;
                    }
                    if x.revents & k::POLLWR != 0 {
                        ev |= 4;
                    }
                    // UNIX's poll reports a closed or failed socket readable: keep the waiter's own mask
                    if x.revents & (k::POLLERR | k::POLLHUP | k::POLLNVAL) != 0 {
                        ev |= want & (1 | 4);
                    }
                    ev
                })
                .collect());
        }
    }

    fn op_env(n: &str) -> Option<String> {
        if NET_SANDBOX {
            return None;
        }
        std::env::var(n).ok()
    }

    // --limit net=MB: every byte the program sends and receives (fsscope.rs counts them)
    fn op_send_counted(id: i64, b: &[u8]) -> Result<i64, NetErr> {
        let r = op_send(id, b);
        if let Ok(k) = r {
            if k > 0 {
                brevi_net_bytes(k as u64);
            }
        }
        r
    }
    fn op_recv_counted(id: i64, n: usize) -> Result<Vec<u8>, NetErr> {
        let r = op_recv(id, n);
        if let Ok(b) = &r {
            brevi_net_bytes(b.len() as u64);
        }
        r
    }

    pub fn ops() -> NetOps {
        NetOps {
            connect: op_connect,
            connect_nb: op_connect_nb,
            ready: op_ready,
            tls: op_tls,
            handshake: op_handshake,
            settimeout: op_settimeout,
            setblocking: op_setblocking,
            send: op_send_counted,
            recv: op_recv_counted,
            fileno: op_fileno,
            close: op_close,
            poll: op_poll,
            env: op_env,
            host_ok,
        }
    }
}
