// ------------------------------------------------------------------------------------------- private folders
// Folders only this user can reach, for what Brevi keeps in a temp folder (privdir.rs). The compiler uses it
// (main.rs tmp_root) and it is pasted into programs that unpack a standalone CPython (pyrt_load.rs), so it uses
// std only. /tmp, and $TMPDIR, may be shared with other local users, who could make a folder there first under
// the name Brevi will use and change what is in it between a build and its run. So a folder is used only when it
// is a real folder (not a link), owned by this user, with no access for group or others, and every folder above
// it can be changed only by this user or root, or is sticky as /tmp is (then no one else can rename or remove
// what this user made in it). Anything else is refused, never repaired or reused. Unix only: on Windows the temp
// folder is already the user's own.
#[cfg(unix)]
#[allow(dead_code)]
pub mod ax_priv {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    use std::path::{Path, PathBuf};
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    pub fn me() -> u32 {
        // SAFETY: geteuid takes nothing and cannot fail.
        unsafe { geteuid() }
    }
    // In a user namespace (a container or sandbox that maps only some users) a folder whose owner is not mapped
    // shows as the overflow user: the host's own system folders (/, /usr, /home) look like that. Such an owner is
    // accepted only there, and only for a folder above the private one.
    fn unmapped() -> Option<u32> {
        let map = std::fs::read_to_string("/proc/self/uid_map").ok()?;
        let whole = map.split_whitespace().collect::<Vec<_>>() == ["0", "0", "4294967295"];
        if whole {
            return None;
        }
        std::fs::read_to_string("/proc/sys/kernel/overflowuid").ok()?.trim().parse().ok()
    }
    // Every folder above `p` (as given and as resolved): owned by this user or root, and writable by no one else
    // unless sticky.
    fn above_ok(p: &Path) -> Result<(), String> {
        let me = me();
        let other = unmapped();
        let given = std::path::absolute(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let real = std::fs::canonicalize(p).map_err(|e| format!("{}: {e}", p.display()))?;
        for top in [given, real] {
            for a in top.ancestors().skip(1) {
                let m = std::fs::metadata(a).map_err(|e| format!("{}: {e}", a.display()))?;
                if m.uid() != me && m.uid() != 0 && Some(m.uid()) != other {
                    return Err(format!("{} belongs to another user (uid {})", a.display(), m.uid()));
                }
                if m.mode() & 0o022 != 0 && m.mode() & 0o1000 == 0 {
                    return Err(format!("{} can be changed by other users and is not sticky", a.display()));
                }
            }
        }
        Ok(())
    }
    // `p` is a private folder of this user: a real folder, its own, mode 0700 or tighter, safe above.
    pub fn check(p: &Path) -> Result<(), String> {
        let m = std::fs::symlink_metadata(p).map_err(|e| format!("{}: {e}", p.display()))?;
        if !m.file_type().is_dir() {
            return Err(format!("{} is not a folder (a link or a file)", p.display()));
        }
        if m.uid() != me() {
            return Err(format!("{} belongs to another user (uid {})", p.display(), m.uid()));
        }
        if m.mode() & 0o077 != 0 {
            return Err(format!("{} is open to other users (mode {:o})", p.display(), m.mode() & 0o7777));
        }
        above_ok(p)
    }
    // Makes `p` (its parent must exist) as a private folder, or takes it if it already is one.
    pub fn make(p: &Path) -> Result<PathBuf, String> {
        match std::fs::DirBuilder::new().mode(0o700).create(p) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(format!("{}: {e}", p.display())),
        }
        check(p).map(|_| p.to_path_buf())
    }
    // This user's private folder for temporary things: `tmp`-UID in the temp folder, else (that refused, say made
    // first by someone else) `cached` in this user's cache folder ($XDG_CACHE_HOME/brevi or ~/.cache/brevi).
    // Err (naming each refusal) when neither is private.
    pub fn root(tmp: &str, cached: &str) -> Result<PathBuf, String> {
        let first = std::env::temp_dir().join(format!("{tmp}-{}", me()));
        let e1 = match make(&first) {
            Ok(p) => return Ok(p),
            Err(e) => e,
        };
        let home = match (std::env::var_os("XDG_CACHE_HOME"), std::env::var_os("HOME")) {
            (Some(x), _) if !x.is_empty() && Path::new(&x).is_absolute() => Some(PathBuf::from(x)),
            (_, Some(h)) if !h.is_empty() && Path::new(&h).is_absolute() => Some(PathBuf::from(h).join(".cache")),
            _ => None,
        };
        let Some(home) = home else {
            return Err(format!("no private temp folder: {e1}; set TMPDIR to a folder only you can write"));
        };
        let brevi = home.join("brevi");
        let second = brevi.join(cached);
        std::fs::create_dir_all(&brevi).map_err(|e| format!("no private temp folder: {e1}; {}: {e}", brevi.display()))?;
        make(&second).map_err(|e2| format!("no private temp folder: {e1}; {e2}; set TMPDIR to a folder only you can write"))
    }
}
