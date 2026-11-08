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
