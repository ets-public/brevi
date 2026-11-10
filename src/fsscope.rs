// ---------- folder-scoped file grants: --allow-read DIR, --allow-write DIR (src/fsscope.rs) ----------
// Pasted into the typed runtime (prelude.rs) and into dynamic builds that touch files (dynpy/mod.rs), so the
// check is the same code everywhere. `roots` are the folders (or files) the caller granted, absolute and with
// symlinks resolved when granted (caps::parse_paths). The path the program names is resolved the same way, a
// file that is not there yet by its folder, and must lie inside one of the roots. The resolved path is what is
// then opened, so a symlink cannot lead outside after the check (the program has no way to make one).
pub fn brevi_scoped(p: &str, roots: &[&str], cap: &str) -> Result<String, String> {
    use std::path::{Component, Path, PathBuf};
    let flag = if cap == "read" { "--allow-read" } else { "--allow-write" };
    let outside = || format!("brevi: --sandbox: '{p}' is outside the folders granted to {cap} ({flag})");
    let raw = Path::new(p);
    if p.is_empty() || p.contains('\0') {
        return Err(outside());
    }
    let abs: PathBuf = if raw.is_absolute() { raw.to_path_buf() } else { std::env::current_dir().map_err(|_| outside())?.join(raw) };
    let real = match std::fs::canonicalize(&abs) {
        Ok(r) => r,
        // Not there (or a dangling link): its folder must be, and its name a plain name that is no link.
        Err(_) => {
            if std::fs::symlink_metadata(&abs).is_ok() {
                return Err(outside());
            }
            let name = match abs.components().last() {
                Some(Component::Normal(n)) => n.to_os_string(),
                _ => return Err(outside()),
            };
            let dir = abs.parent().ok_or_else(outside)?;
            std::fs::canonicalize(dir).map_err(|_| outside())?.join(name)
        }
    };
    if roots.iter().any(|r| real.starts_with(r)) {
        real.to_str().map(String::from).ok_or_else(outside)
    } else {
        Err(outside())
    }
}

// --limit files=N, net=MB and written=MB (proc.rs passes them as BREVI_LIMIT_FILES, BREVI_LIMIT_NET and
// BREVI_LIMIT_WRITTEN, read once): the files the program creates, the bytes it sends and receives, and the bytes
// it writes to files, counted where Brevi's own code creates a file, moves bytes or writes them. Past any, the
// program stops as a kernel limit stops it: one JSON line on stderr, exit 124.
pub fn brevi_limit_stop(on: &str, limit: u64, unit: &str, msg: &str) -> ! {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    eprintln!("{{\"e\":\"limit\",\"on\":\"{on}\",\"limit\":\"{limit} {unit}\",\"msg\":\"{msg}\",\"fix\":\"raise it with --limit {on}=... if the program needs more\"}}");
    std::process::exit(124);
}

fn brevi_limit_env(i: usize) -> Option<u64> {
    static L: [std::sync::OnceLock<Option<u64>>; 3] = [std::sync::OnceLock::new(), std::sync::OnceLock::new(), std::sync::OnceLock::new()];
    *L[i].get_or_init(|| std::env::var(["BREVI_LIMIT_FILES", "BREVI_LIMIT_NET", "BREVI_LIMIT_WRITTEN"][i]).ok().and_then(|v| v.parse().ok()))
}

// Before the program's code creates (or may create) the file at `p`.
pub fn brevi_new_file(p: &str) {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if let Some(l) = brevi_limit_env(0) {
        if std::fs::symlink_metadata(p).is_err() && N.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1 > l {
            brevi_limit_stop("files", l, "files", &format!("the program tried to create more than {l} files"));
        }
    }
}

// After the program sent or received `n` bytes over the network.
pub fn brevi_net_bytes(n: u64) {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if let Some(l) = brevi_limit_env(1) {
        if N.fetch_add(n, std::sync::atomic::Ordering::SeqCst) + n > l {
            brevi_limit_stop("net", l, "bytes", &format!("the program sent and received more than {l} bytes over the network"));
        }
    }
}

// Whether --limit written is set: native libraries that write files on their own (SQLite, DuckDB, ONNX Runtime)
// then write none, since their bytes would not pass through brevi_written.
pub fn brevi_written_held() -> bool {
    brevi_limit_env(2).is_some()
}

// Before the program's code writes `n` bytes to a file. The check comes first, so a write that would pass the
// limit never starts: the limit holds to the byte, however large one write is.
pub fn brevi_written(n: u64) {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if let Some(l) = brevi_limit_env(2) {
        let before = N.fetch_add(n, std::sync::atomic::Ordering::SeqCst);
        if before.saturating_add(n) > l {
            brevi_limit_stop("written", l, "bytes", &format!("the program tried to write more than {l} bytes to files"));
        }
    }
}
