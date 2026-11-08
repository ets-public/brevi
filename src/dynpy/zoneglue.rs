// ---------- the time zone database, for Brevi's zoneinfo (pasted into the program; see ZONE_FS in rt_zone.rs) ----------
// Reads TZif files under the TZPATH folders (zoneinfo checks each key first: no absolute paths, no ".."), and for
// tzlocal the TZ variable and the system's zone settings (/etc/timezone, /etc/localtime and the like).
mod brevi_zoneglue {
    use brevi_dyn::ZoneOps;

    fn read(p: &str) -> Option<Vec<u8>> {
        std::fs::read(p).ok()
    }

    fn is_file(p: &str) -> bool {
        std::fs::metadata(p).is_ok_and(|m| m.is_file())
    }

    fn walk(root: &str) -> Option<Vec<String>> {
        if !std::path::Path::new(root).exists() {
            return None;
        }
        let mut out = vec![];
        let mut stack = vec![String::new()];
        while let Some(rel) = stack.pop() {
            let dir = if rel.is_empty() { root.to_string() } else { format!("{root}/{rel}") };
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let key = if rel.is_empty() { name } else { format!("{rel}/{name}") };
                // os.walk follows the folder's entries; symlinks to files count as files
                match std::fs::metadata(e.path()) {
                    Ok(m) if m.is_dir() => stack.push(key),
                    Ok(m) if m.is_file() => out.push(key),
                    _ => {}
                }
            }
        }
        Some(out)
    }

    fn env(k: &str) -> Option<String> {
        std::env::var(k).ok()
    }

    fn exists(p: &str) -> bool {
        std::path::Path::new(p).exists()
    }

    fn is_link(p: &str) -> bool {
        std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink())
    }

    // os.path.realpath: the canonical path, or the path as given when it cannot be resolved
    fn realpath(p: &str) -> String {
        std::fs::canonicalize(p).map(|x| x.to_string_lossy().into_owned()).unwrap_or_else(|_| p.to_string())
    }

    pub fn ops() -> ZoneOps {
        ZoneOps { read, is_file, walk, env, exists, is_link, realpath }
    }
}
