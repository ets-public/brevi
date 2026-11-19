// --allow-read DIR / --allow-write DIR as programs run it (src/fsscope.rs brevi_scoped, pasted into every built
// program). The fuzzer names paths, relative or absolute, against a granted folder holding links that lead out of
// it. Property: a path the check lets through resolves inside the granted folder.
#![no_main]
use libfuzzer_sys::fuzz_target;
use std::path::PathBuf;
use std::sync::OnceLock;

fn tree() -> &'static PathBuf {
    static R: OnceLock<PathBuf> = OnceLock::new();
    R.get_or_init(|| {
        let base = std::env::temp_dir().join(format!("brevi-fuzz-scope-{}", std::process::id()));
        let granted = base.join("granted");
        std::fs::create_dir_all(granted.join("sub/deeper")).unwrap();
        std::fs::create_dir_all(base.join("outside")).unwrap();
        std::fs::write(granted.join("a.txt"), "a").unwrap();
        std::fs::write(base.join("outside/secret.txt"), "s").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let _ = symlink(base.join("outside"), granted.join("out"));
            let _ = symlink("../outside/secret.txt", granted.join("sub/secret"));
            let _ = symlink("deeper", granted.join("sub/in"));
            let _ = symlink(base.join("nowhere"), granted.join("dangling"));
            let _ = symlink(granted.join("loop"), granted.join("loop"));
        }
        let granted = std::fs::canonicalize(&granted).unwrap();
        // relative paths resolve against the granted folder's parent
        std::env::set_current_dir(&base).unwrap();
        granted
    })
}

fuzz_target!(|p: &str| {
    let g = tree();
    let gs = g.to_str().unwrap();
    let p = p.replace("$G", gs);
    for cap in ["read", "write"] {
        if let Ok(real) = brevi_fuzz::pasted::fsscope::brevi_scoped(&p, &[gs], cap) {
            let real = PathBuf::from(real);
            assert!(real.starts_with(g), "{p:?} -> {real:?} is outside {g:?}");
            if let Ok(again) = std::fs::canonicalize(&real) {
                assert!(again.starts_with(g), "{p:?} -> {real:?} -> {again:?} is outside {g:?}");
            }
        }
    }
});
