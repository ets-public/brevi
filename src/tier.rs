// Tiered builds for the edit loop (docs/LATENCY.md): `brevi run` and `brevi test` after an edit build the
// program at opt-level 0 (about 0.35 s instead of 0.85 s for opt-level 1 on bench/latency) and run that binary
// at once. In the same call a background job builds the same Rust at opt-level 1 into the build cache, under
// the key a plain opt-level 1 build uses, so the next run of the unchanged program gets the optimised binary.
//
//   - `brevi build` (what you ship) and `--release` are never tiered.
//   - Only with the build cache on: with BREVI_NO_CACHE=1 or --no-cache, nothing could pick the
//     background build up, so the build is at opt-level 1 as before.
//   - BREVI_TIERED=0 turns tiering off.
//
// The background job is this binary again (`brevi __tier DEST WORK -- RUSTC ARGS...`), started detached with
// no stdin, stdout or stderr (a caller reading the run's output must not wait for it) and at low priority.
// It builds into its own folder under the cache, then renames the binary into place, so a reader sees the
// whole file or nothing.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

// Set by `brevi run` and `brevi test` (and by the child build of a cached run, through BREVI_EDIT_LOOP=1).
static EDIT_LOOP: AtomicBool = AtomicBool::new(false);

pub fn edit_loop() {
    EDIT_LOOP.store(true, Ordering::Relaxed);
}

// The variable the parent of a cached run sets on its child build.
pub const ENV: (&str, &str) = ("BREVI_EDIT_LOOP", "1");

// True when this build may be made at opt-level 0 with opt-level 1 in the background.
pub fn on() -> bool {
    (EDIT_LOOP.load(Ordering::Relaxed) || std::env::var(ENV.0).is_ok_and(|v| v == ENV.1))
        && crate::runcache::on()
        && std::env::var("BREVI_TIERED").map_or(true, |v| v.trim() != "0")
}

// `extra` with opt-level 1 swapped for opt-level 0, when it holds opt-level 1.
pub fn quick_flags<'a>(extra: &[&'a str]) -> Option<Vec<&'a str>> {
    if !extra.contains(&"opt-level=1") {
        return None;
    }
    Some(extra.iter().map(|e| if *e == "opt-level=1" { "opt-level=0" } else { *e }).collect())
}

// Starts the opt-level 1 build of `rust` in the background, to land at `dest` (a build-cache path). `args` are
// rustc's arguments without the source file and `-o`. Nothing happens when another job for `dest` started in
// the last ten minutes. Afterwards, in a child build of the result cache, tells the parent where the
// optimised binary will be (runcache::upgrade_at), so its entry picks it up once it lands.
pub fn background(dest: &Path, rust: &str, args: &[String]) {
    crate::runcache::upgrade_at(dest);
    if dest.exists() {
        return;
    }
    let Some(cdir) = dest.parent() else { return };
    let name = dest.file_name().unwrap_or_default().to_string_lossy().to_string();
    // `.tmp-` names are skipped by the cache's size pruning while the job runs.
    let work = cdir.join(format!(".tmp-tier-{name}"));
    if let Ok(m) = std::fs::metadata(&work) {
        let age = m.modified().ok().and_then(|t| t.elapsed().ok()).map_or(0, |d| d.as_secs());
        if age < 600 {
            return;
        }
        std::fs::remove_dir_all(&work).ok();
    }
    if std::fs::create_dir_all(&work).is_err() || std::fs::write(work.join("main.rs"), rust).is_err() {
        std::fs::remove_dir_all(&work).ok();
        return;
    }
    let me = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("brevi"));
    let mut c = Command::new(me);
    c.arg("__tier").arg(dest).arg(&work).arg("--").arg(crate::rustc_cmd().get_program()).args(args);
    // Run from its own folder: it must not hold the caller's (a folder an MCP call made, deleted afterwards).
    c.current_dir(&work).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    detach(&mut c);
    if c.spawn().is_err() {
        std::fs::remove_dir_all(&work).ok();
    }
}

// The background job: `brevi __tier DEST WORK -- RUSTC ARGS...`. Never returns.
pub fn job(argv: &[String]) -> ! {
    let (Some(dest), Some(work)) = (argv.first().map(PathBuf::from), argv.get(1).map(PathBuf::from)) else { std::process::exit(2) };
    let Some(rustc) = argv.get(3) else { std::process::exit(2) };
    let out = work.join("prog");
    let ok = Command::new(rustc)
        .args(&argv[4..])
        .arg("-o")
        .arg(&out)
        .arg(work.join("main.rs"))
        .current_dir(&work)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    let landed = ok && std::fs::rename(&out, &dest).is_ok();
    std::fs::remove_dir_all(&work).ok();
    if landed {
        if let Some(d) = dest.parent() {
            crate::runcache::prune(d);
        }
    }
    std::process::exit(if landed { 0 } else { 1 })
}

// Not tied to the caller: no parent-death signal, its own process group (a Ctrl-C or a `timeout` that stops
// the run leaves the build to finish), and a low priority, so the program being run gets the CPU first.
fn detach(c: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
        unsafe {
            c.pre_exec(|| {
                setpriority(0, 0, 10); // PRIO_PROCESS, this process, nice 10
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | BELOW_NORMAL_PRIORITY_CLASS
        c.creation_flags(0x0000_0008 | 0x0000_0200 | 0x0000_4000);
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn setpriority(which: i32, who: u32, prio: i32) -> i32;
}

#[cfg(test)]
mod tests {
    #[test]
    fn quick_flags_swap_only_opt_level_1() {
        assert_eq!(super::quick_flags(&["--test", "-C", "opt-level=1"]), Some(vec!["--test", "-C", "opt-level=0"]));
        assert_eq!(super::quick_flags(&["-C", "opt-level=3"]), None);
    }
}
