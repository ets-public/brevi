// `brevi mcp --http [HOST:]PORT`: the same MCP server over Streamable HTTP (MCP 2025-03-26 on), for a
// client that cannot start a process: one inside a hosted sandbox (E2B, Daytona, Modal, Cloudflare), reached
// through the platform's port forwarding.
//
// The HTTP side is a front for the stdio server: it starts `brevi mcp` (same flags) as its child and passes
// each JSON-RPC message through, so every tool, rule and limit is the stdio server's. Each POST carries one
// message. A request is answered with its reply as an application/json body (the transport's JSON response
// mode); a notification or a client's reply gets 202 Accepted. Request ids are renamed on the way in, so
// clients that use the same ids cannot get each other's replies, and named back on the way out. No GET
// stream: the server sends nothing on its own, so GET is 405 as the spec allows.
//
// The server runs whatever program a client sends, so who may reach it matters:
// - It listens on 127.0.0.1 unless told otherwise, and any other address needs a bearer token (--token T, or
//   BREVI_MCP_TOKEN), checked on every request (401 without it).
// - A request with an Origin header is refused (403) unless the origin is local (http://localhost,
//   http://127.0.0.1, http://[::1], any port) or allowed with --allow-origin, so a web page cannot use a
//   local server (DNS rebinding).
// - Bodies are limited to 4 MiB (413).
use crate::json::{parse, J};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

const MAX_BODY: usize = 4 * 1024 * 1024;

struct Front {
    token: Option<String>,
    origins: Vec<String>,
    path: String,
    child: Mutex<ChildStdin>,
    // renamed id -> (where the reply goes, the client's own id)
    pending: Mutex<HashMap<String, (mpsc::Sender<String>, J)>>,
    next: AtomicU64,
}

const USAGE: &str = "usage: brevi mcp --http [HOST:]PORT [--token TOKEN] [--path /mcp] [--allow-origin ORIGIN]... [--timeout SECONDS] [--mem-mb MB]";

pub fn serve(argv: &[String]) -> ! {
    let mut addr = None;
    let mut token = std::env::var("BREVI_MCP_TOKEN").ok().filter(|t| !t.is_empty());
    let mut path = "/mcp".to_string();
    let mut origins = vec![];
    let mut rest = vec![];
    let mut i = 0;
    while i < argv.len() {
        let v = argv.get(i + 1).cloned();
        match (argv[i].as_str(), v) {
            ("--http", Some(a)) => addr = Some(a),
            ("--token", Some(t)) => token = Some(t),
            ("--path", Some(p)) if p.starts_with('/') => path = p,
            ("--allow-origin", Some(o)) => origins.push(o.trim_end_matches('/').to_string()),
            ("--timeout" | "--mem-mb", Some(v)) => {
                rest.push(argv[i].clone());
                rest.push(v);
            }
            _ => fail(USAGE),
        }
        i += 2;
    }
    let addr = addr.unwrap_or_else(|| fail(USAGE));
    let addr = if addr.parse::<u16>().is_ok() { format!("127.0.0.1:{addr}") } else { addr };
    let host = addr.rsplit_once(':').map_or("", |(h, _)| h).trim_matches(|c| c == '[' || c == ']');
    let local = matches!(host, "127.0.0.1" | "localhost" | "::1");
    if !local && token.is_none() {
        fail(&format!("brevi mcp --http {addr}: listening beyond this machine needs a bearer token (--token TOKEN or BREVI_MCP_TOKEN), since a client can run programs"));
    }
    if token.as_deref().is_some_and(|t| t.len() < 16) {
        fail("brevi mcp --http: the token must be at least 16 characters");
    }
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| fail(&format!("brevi mcp --http: cannot listen on {addr}: {e}")));

    let exe = std::env::current_exe().unwrap_or_else(|e| fail(&format!("brevi mcp --http: {e}")));
    let mut child = Command::new(exe)
        .arg("mcp")
        .args(&rest)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap_or_else(|e| fail(&format!("brevi mcp --http: cannot start brevi mcp: {e}")));
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let front = Arc::new(Front { token, origins, path, child: Mutex::new(stdin), pending: Mutex::new(HashMap::new()), next: AtomicU64::new(0) });

    // Replies from the stdio server, back to the POST waiting for each.
    {
        let front = front.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let Some(mut msg) = parse(&line) else { continue };
                let Some(J::Str(id)) = msg.get("id").cloned() else { continue };
                let Some((tx, orig)) = front.pending.lock().unwrap().remove(&id) else { continue };
                set(&mut msg, "id", orig);
                let _ = tx.send(crate::mcp::to_json(&msg));
            }
            // The stdio server stopped (it never does on its own while its stdin is open).
            let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(1);
            std::process::exit(code);
        });
    }
    eprintln!("brevi mcp: Streamable HTTP on http://{addr}{}{}", front.path, if front.token.is_some() { " (bearer token required)" } else { "" });
    for conn in listener.incoming() {
        let Ok(conn) = conn else { continue };
        let front = front.clone();
        std::thread::spawn(move || connection(&front, conn));
    }
    std::process::exit(0);
}

fn fail(m: &str) -> ! {
    eprintln!("{m}");
    std::process::exit(2);
}

fn set(msg: &mut J, k: &str, v: J) {
    if let J::Obj(kv) = msg {
        match kv.iter_mut().find(|(key, _)| key == k) {
            Some(e) => e.1 = v,
            None => kv.push((k.to_string(), v)),
        }
    }
}

struct Req {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    close: bool,
}

impl Req {
    fn header(&self, k: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
    }
}

enum Read_ {
    Req(Req),
    Bad(u16, &'static str),
    Gone,
}

fn read_req(r: &mut BufReader<TcpStream>) -> Read_ {
    let mut line = String::new();
    // a blank line before the request line is allowed (RFC 9112 2.2)
    loop {
        line.clear();
        match r.by_ref().take(8192).read_line(&mut line) {
            Ok(0) | Err(_) => return Read_::Gone,
            Ok(_) => {}
        }
        if !line.trim().is_empty() {
            break;
        }
    }
    let first: Vec<String> = line.split_whitespace().map(String::from).collect();
    let [method, target, version] = &first[..] else { return Read_::Bad(400, "Bad Request") };
    let (method, target, version) = (method.clone(), target.clone(), version.clone());
    let mut headers = vec![];
    let mut size = 0;
    loop {
        line.clear();
        match r.by_ref().take(8192).read_line(&mut line) {
            Ok(0) | Err(_) => return Read_::Gone,
            Ok(n) => size += n,
        }
        if size > 64 * 1024 {
            return Read_::Bad(431, "Request Header Fields Too Large");
        }
        let l = line.trim_end_matches(['\r', '\n']);
        if l.is_empty() {
            break;
        }
        let Some((k, v)) = l.split_once(':') else { return Read_::Bad(400, "Bad Request") };
        headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
    }
    let get = |k: &str| headers.iter().find(|(n, _)| n == k).map(|(_, v)| v.to_ascii_lowercase());
    let close = match get("connection") {
        Some(c) if c.contains("close") => true,
        Some(c) if c.contains("keep-alive") => false,
        _ => version == "HTTP/1.0",
    };
    let mut body = vec![];
    if get("transfer-encoding").is_some_and(|t| t.contains("chunked")) {
        loop {
            line.clear();
            if r.by_ref().take(1024).read_line(&mut line).unwrap_or(0) == 0 {
                return Read_::Gone;
            }
            let Ok(n) = usize::from_str_radix(line.trim().split(';').next().unwrap_or("").trim(), 16) else { return Read_::Bad(400, "Bad Request") };
            if body.len() + n > MAX_BODY {
                return Read_::Bad(413, "Payload Too Large");
            }
            if n == 0 {
                // trailers, then the blank line
                loop {
                    line.clear();
                    if r.by_ref().take(8192).read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                        break;
                    }
                }
                break;
            }
            let at = body.len();
            body.resize(at + n, 0);
            if r.read_exact(&mut body[at..]).is_err() {
                return Read_::Gone;
            }
            let mut crlf = [0u8; 2];
            if r.read_exact(&mut crlf).is_err() {
                return Read_::Gone;
            }
        }
    } else if let Some(n) = get("content-length") {
        let Ok(n) = n.trim().parse::<usize>() else { return Read_::Bad(400, "Bad Request") };
        if n > MAX_BODY {
            return Read_::Bad(413, "Payload Too Large");
        }
        body.resize(n, 0);
        if r.read_exact(&mut body).is_err() {
            return Read_::Gone;
        }
    }
    let path = target.split('?').next().unwrap_or("").to_string();
    Read_::Req(Req { method, path, headers, body, close })
}

fn reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        _ => "Error",
    }
}

fn respond(w: &mut TcpStream, code: u16, extra: &[(&str, &str)], body: &str, close: bool) -> bool {
    let mut h = format!("HTTP/1.1 {code} {}\r\n", reason(code));
    if !body.is_empty() {
        h.push_str("content-type: application/json\r\n");
    }
    for (k, v) in extra {
        h.push_str(&format!("{k}: {v}\r\n"));
    }
    h.push_str(&format!("content-length: {}\r\n", body.len()));
    if close {
        h.push_str("connection: close\r\n");
    }
    h.push_str("\r\n");
    h.push_str(body);
    w.write_all(h.as_bytes()).and_then(|_| w.flush()).is_ok()
}

// A JSON-RPC error with no id, as the SDK's transports answer an HTTP-level refusal.
fn error_body(code: i32, msg: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":null,"error":{{"code":{code},"message":"{}"}}}}"#, crate::diag::esc(msg))
}

fn local_origin(o: &str) -> bool {
    ["http://localhost", "http://127.0.0.1", "http://[::1]", "https://localhost", "https://127.0.0.1", "https://[::1]"].iter().any(|p| o == *p || o.strip_prefix(p).is_some_and(|r| r.starts_with(':')))
}

// Compares in time that does not depend on where the strings first differ.
fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

fn connection(front: &Front, conn: TcpStream) {
    let _ = conn.set_read_timeout(Some(Duration::from_secs(300)));
    let Ok(mut w) = conn.try_clone() else { return };
    let mut r = BufReader::new(conn);
    loop {
        let req = match read_req(&mut r) {
            Read_::Req(q) => q,
            Read_::Bad(code, m) => {
                respond(&mut w, code, &[], &error_body(-32600, m), true);
                return;
            }
            Read_::Gone => return,
        };
        let close = req.close;
        let (code, extra, body) = answer(front, &req);
        let extra: Vec<(&str, &str)> = extra.iter().map(|(k, v)| (*k, v.as_str())).collect();
        if !respond(&mut w, code, &extra, &body, close) || close {
            return;
        }
    }
}

fn answer(front: &Front, req: &Req) -> (u16, Vec<(&'static str, String)>, String) {
    if req.path != front.path {
        return (404, vec![], error_body(-32600, "Not Found"));
    }
    if let Some(o) = req.header("origin") {
        let o = o.trim_end_matches('/');
        if !local_origin(o) && !front.origins.iter().any(|x| x == o) {
            return (403, vec![], error_body(-32600, "Forbidden: Origin not allowed"));
        }
    }
    if let Some(t) = &front.token {
        let ok = req.header("authorization").and_then(|a| a.strip_prefix("Bearer ").or_else(|| a.strip_prefix("bearer "))).is_some_and(|g| same(g.trim(), t));
        if !ok {
            return (401, vec![("www-authenticate", "Bearer".into())], error_body(-32600, "Unauthorized: a bearer token is required"));
        }
    }
    match req.method.as_str() {
        "POST" => {}
        "GET" | "DELETE" => return (405, vec![("allow", "POST".into())], error_body(-32600, "Method Not Allowed: this server answers each POST with JSON and keeps no streams or sessions")),
        _ => return (405, vec![("allow", "POST".into())], error_body(-32600, "Method Not Allowed")),
    }
    let ct = req.header("content-type").unwrap_or("").split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    if ct != "application/json" {
        return (415, vec![], error_body(-32600, "Unsupported Media Type: Content-Type must be application/json"));
    }
    if let Some(a) = req.header("accept") {
        let ok = a.split(',').map(|t| t.split(';').next().unwrap_or("").trim().to_ascii_lowercase()).any(|t| t == "application/json" || t == "application/*" || t == "*/*");
        if !ok {
            return (406, vec![], error_body(-32600, "Not Acceptable: Client must accept application/json"));
        }
    }
    let Some(mut msg) = std::str::from_utf8(&req.body).ok().and_then(parse) else {
        return (400, vec![], error_body(-32700, "Parse error: the body is not valid JSON"));
    };
    if !matches!(msg, J::Obj(_)) {
        let m = if matches!(msg, J::Arr(_)) { "batches are not supported: send one message per request" } else { "a message must be a JSON object" };
        return (400, vec![], error_body(-32600, m));
    }
    let id = msg.get("id").cloned();
    let is_request = msg.get("method").is_some() && !matches!(id, None | Some(J::Null));
    if !is_request {
        // a notification (or a reply, which the stdio server ignores): cancelling names the renamed id
        if msg.get("method").and_then(|m| m.str()) == Some("notifications/cancelled") {
            if let Some(rid) = msg.get("params").and_then(|p| p.get("requestId")).cloned() {
                let want = crate::mcp::to_json(&rid);
                let renamed = front.pending.lock().unwrap().iter().filter(|(_, (_, o))| crate::mcp::to_json(o) == want).map(|(k, _)| k.clone()).max_by_key(|k| k[2..].parse::<u64>().unwrap_or(0));
                if let Some(r) = &renamed {
                    // the waiting POST ends (202) whether or not a reply was already on its way
                    front.pending.lock().unwrap().remove(r);
                }
                if let (Some(r), J::Obj(kv)) = (renamed, &mut msg) {
                    if let Some((_, J::Obj(p))) = kv.iter_mut().find(|(k, _)| k == "params") {
                        if let Some(e) = p.iter_mut().find(|(k, _)| k == "requestId") {
                            e.1 = J::Str(r);
                        }
                    }
                }
            }
        }
        if !write_line(front, &crate::mcp::to_json(&msg)) {
            return (500, vec![], error_body(-32603, "the MCP server has stopped"));
        }
        return (202, vec![], String::new());
    }
    let renamed = format!("h-{}", front.next.fetch_add(1, Ordering::SeqCst));
    let (tx, rx) = mpsc::channel();
    front.pending.lock().unwrap().insert(renamed.clone(), (tx, id.unwrap_or(J::Null)));
    set(&mut msg, "id", J::Str(renamed.clone()));
    if !write_line(front, &crate::mcp::to_json(&msg)) {
        front.pending.lock().unwrap().remove(&renamed);
        return (500, vec![], error_body(-32603, "the MCP server has stopped"));
    }
    match rx.recv() {
        Ok(reply) => (200, vec![], reply),
        // cancelled: the stdio server sends no reply, so neither does this one (the SDK's 2025 transports
        // answer -32800 instead; a client that cancelled has stopped waiting)
        Err(_) => (202, vec![], String::new()),
    }
}

fn write_line(front: &Front, line: &str) -> bool {
    let mut w = front.child.lock().unwrap();
    writeln!(w, "{line}").and_then(|_| w.flush()).is_ok()
}
