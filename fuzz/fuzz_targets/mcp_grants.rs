// The MCP server's grant policy (src/mcp.rs grants): a tools/call's arguments, as the model writes them, against
// what the person who started `brevi mcp` allowed. Property: without --trust-calls, a call never runs unsandboxed
// and every flag it passes on to the build is inside the server's ceiling, checked here again from the flags
// themselves (paths resolved on disk, so a symlink out of a granted folder must be refused).
#![no_main]
use brevi_fuzz::api::*;
use libfuzzer_sys::fuzz_target;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// A granted folder with a link that leads out of it, and a folder that is not granted.
fn root() -> &'static (PathBuf, PathBuf) {
    static R: OnceLock<(PathBuf, PathBuf)> = OnceLock::new();
    R.get_or_init(|| {
        let base = std::env::temp_dir().join(format!("brevi-fuzz-grants-{}", std::process::id()));
        let (granted, outside) = (base.join("granted"), base.join("outside"));
        std::fs::create_dir_all(granted.join("sub")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink(&outside, granted.join("out"));
            let _ = std::os::unix::fs::symlink("/", granted.join("top"));
        }
        (std::fs::canonicalize(granted).unwrap(), std::fs::canonicalize(outside).unwrap())
    })
}

fn inside(ceiling: &[String], cap: &str, p: &str) -> bool {
    if ceiling.iter().any(|c| c == cap) {
        return true;
    }
    let real = std::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p));
    ceiling.iter().filter_map(|c| c.strip_prefix(cap).and_then(|r| r.strip_prefix(':'))).any(|r| real.starts_with(Path::new(r)))
}

fuzz_target!(|data: &[u8]| {
    let Some((&sel, rest)) = data.split_first() else { return };
    let Ok(src) = std::str::from_utf8(rest) else { return };
    let (granted, _) = root();
    let g = granted.to_str().unwrap();
    // Relative paths in a call resolve against where brevi mcp started: here, the granted folder's parent.
    let _ = std::env::set_current_dir(granted.parent().unwrap());
    let src = src.replace("$G", g);
    let Some(a) = json_parse(&src) else { return };
    let ceilings: [Vec<String>; 5] = [
        vec![],
        vec![format!("read:{g}")],
        vec![format!("read:{g}"), format!("write:{g}/sub")],
        vec!["net:example.com".into(), "py:json".into()],
        vec!["read".into(), "net".into()],
    ];
    let ceiling = &ceilings[sel as usize % ceilings.len()];
    let name = ["run", "check", "guide", "show", "verify"][(sel as usize / 5) % 5];
    let Ok((sandboxed, flags)) = mcp_grants(name, &a, false, ceiling) else { return };
    if !sandboxed {
        assert!(name == "guide" || name == "show", "{name} ran unsandboxed without --trust-calls: {src}");
        return;
    }
    assert_eq!(flags.first().map(String::as_str), Some("--sandbox"));
    for pair in flags[1..].chunks(2) {
        let (flag, value) = (pair[0].as_str(), pair[1].as_str());
        for w in value.split(',').map(str::trim).filter(|w| !w.is_empty()) {
            let ok = match flag {
                "--allow" => w == "none" || ceiling.iter().any(|c| c == w),
                "--allow-py" => ceiling.iter().any(|c| *c == format!("py:{}", w.split('.').next().unwrap())),
                "--allow-net" => ceiling.iter().any(|c| c == "net" || *c == format!("net:{}", w.to_ascii_lowercase())),
                "--allow-read" => inside(ceiling, "read", w),
                "--allow-write" => inside(ceiling, "write", w),
                f => panic!("unknown flag {f}"),
            };
            assert!(ok, "{flag} {w} passed a ceiling of {ceiling:?}: {src}");
        }
    }
});
