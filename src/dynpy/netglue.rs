
// ---------- the network, for Brevi's requests and httpx (pasted into the program; see rt_net.rs) ----------
// Sockets are the system's (connect, poll), names resolve with getaddrinfo, and TLS is the system's OpenSSL,
// loaded when a program first makes an HTTPS connection, as CPython's ssl module uses it. Errors carry
// CPython's messages ("[Errno 111] Connection refused", "timed out"). Under --sandbox the program reaches only
// the hosts the caller granted (NET_ANY / NET_HOSTS, set by the compiler) and reads no environment variables.
#[allow(non_snake_case, non_camel_case_types, dead_code)]
mod brevi_netglue {
    use super::*;
    use brevi_dyn::{NetErr, NetOps};
    use std::ffi::{c_char, c_int, c_long, c_void, CStr, CString};
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::sync::Mutex;
    use std::collections::HashMap;
    use std::time::Duration;

    type P = *mut c_void;

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

    #[repr(C)]
    struct PollFd {
        fd: c_int,
        events: i16,
        revents: i16,
    }

    #[cfg(target_os = "linux")]
    mod k {
        pub const AF_INET6: u16 = 10;
        pub const SOL_SOCKET: i32 = 1;
        pub const SO_ERROR: i32 = 4;
        pub const O_NONBLOCK: i32 = 0o4000;
        pub const EINPROGRESS: i32 = 115;
        pub const SOCK_CLOEXEC: i32 = 0o2000000;
        pub const SIN_LEN: bool = false;
    }
    #[cfg(not(target_os = "linux"))]
    mod k {
        pub const AF_INET6: u16 = 30;
        pub const SOL_SOCKET: i32 = 0xffff;
        pub const SO_ERROR: i32 = 0x1007;
        pub const O_NONBLOCK: i32 = 4;
        pub const EINPROGRESS: i32 = 36;
        pub const SOCK_CLOEXEC: i32 = 0;
        pub const SIN_LEN: bool = true;
    }
    const IPPROTO_TCP: i32 = 6;
    const TCP_NODELAY: i32 = 1;

    struct Conn {
        // None while a non-blocking connect is in progress
        s: Option<TcpStream>,
        pending: i32,
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
    fn resolve(host: &str, port: u16) -> Result<Vec<SocketAddr>, NetErr> {
        let h = host.trim_start_matches('[').trim_end_matches(']');
        match (h, port).to_socket_addrs() {
            Ok(a) => Ok(a.collect()),
            Err(e) => {
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
                Err(NetErr { kind: "socket.gaierror", msg: format!("[Errno {code}] {m}") })
            }
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

    // Starts a non-blocking connect: the socket, or the error.
    fn start(a: &SocketAddr) -> Result<i32, NetErr> {
        let dom = if a.is_ipv4() { 2 } else { k::AF_INET6 as i32 };
        let fd = unsafe { socket(dom, 1 | k::SOCK_CLOEXEC, 0) };
        if fd < 0 {
            return Err(errno_err(std::io::Error::last_os_error().raw_os_error().unwrap_or(0)));
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
            let e = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            if e != k::EINPROGRESS {
                unsafe { close(fd) };
                return Err(errno_err(e));
            }
        }
        Ok(fd)
    }

    fn so_error(fd: i32) -> i32 {
        let mut e: c_int = 0;
        let mut l: u32 = 4;
        unsafe { getsockopt(fd, k::SOL_SOCKET, k::SO_ERROR, &mut e as *mut c_int as *mut c_void, &mut l) };
        e
    }

    fn wait_fd(fd: i32, events: i16, timeout: f64) -> Result<bool, NetErr> {
        let mut p = PollFd { fd, events, revents: 0 };
        let ms = if timeout < 0.0 { -1 } else { (timeout * 1000.0).ceil() as c_int };
        loop {
            let r = unsafe { poll(&mut p, 1, ms) };
            if r < 0 {
                let e = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
                if e == 4 {
                    continue;
                }
                return Err(errno_err(e));
            }
            return Ok(r > 0);
        }
    }

    fn finish(fd: i32, timeout: f64) -> Conn {
        let s = unsafe { TcpStream::from_raw_fd(fd) };
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
                        unsafe { close(fd) };
                        last = Some(timed_out(false, "connect"));
                        continue;
                    }
                    let e = so_error(fd);
                    if e != 0 {
                        unsafe { close(fd) };
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
                let s = unsafe { TcpStream::from_raw_fd(c.pending) };
                s.set_nonblocking(true).ok();
                c.s = Some(s);
                c.pending = -1;
                return Ok(true);
            }
            unsafe { close(c.pending) };
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
        SSL.get_or_init(|| unsafe { load_ssl() }).as_ref().ok_or(NetErr {
            kind: "ssl.SSLError",
            msg: "brevi: HTTPS needs the system's OpenSSL (libssl.so.3 or libssl.so.1.1), which was not found".into(),
        })
    }

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
            let fd = c.s.as_ref().map_or(-1, |x| x.as_raw_fd());
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
                    let os = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
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
                            let os = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
                            unsafe { (s.ERR_clear_error)() };
                            if os != 0 && os != 11 && os != 35 {
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
        with(id, |c| Ok(c.s.as_ref().map_or(c.pending as i64, |s| s.as_raw_fd() as i64))).unwrap_or(-1)
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
                unsafe { close(c.pending) };
            }
        }
    }

    fn op_poll(fds: &[(i64, i64)], timeout: f64) -> Result<Vec<i64>, NetErr> {
        let mut p: Vec<PollFd> = fds.iter().map(|&(fd, ev)| PollFd { fd: fd as c_int, events: ev as i16, revents: 0 }).collect();
        let ms = if timeout < 0.0 { -1 } else { (timeout * 1000.0).ceil() as c_int };
        loop {
            let r = unsafe { poll(p.as_mut_ptr(), p.len() as u64, ms) };
            if r < 0 {
                let e = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
                if e == 4 {
                    continue;
                }
                return Err(errno_err(e));
            }
            return Ok(p.iter().map(|x| x.revents as i64).collect());
        }
    }

    fn op_env(n: &str) -> Option<String> {
        if NET_SANDBOX {
            return None;
        }
        std::env::var(n).ok()
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
            send: op_send,
            recv: op_recv,
            fileno: op_fileno,
            close: op_close,
            poll: op_poll,
            env: op_env,
            host_ok,
        }
    }
}
