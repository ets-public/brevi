// Linux: `--limit threads` held by the kernel. The program runs in a cgroup of its own whose pids.max is the
// limit, so the kernel refuses each thread or process past it (clone fails with EAGAIN), in the program and in
// everything it starts, from its first instruction: the program moves itself in between fork and exec. Where
// Brevi cannot create such a cgroup (no pids controller, or no write access to its own cgroup) the limit is
// refused, never polled.
#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;

unsafe extern "C" {
    fn write(fd: i32, buf: *const u8, n: usize) -> isize;
    fn kill(pid: i32, sig: i32) -> i32;
}

// The cgroup folder this process is in for the pids controller: cgroup v1's pids hierarchy when it is mounted
// (hybrid systems keep the controller there), else the unified (v2) hierarchy.
fn own() -> Result<(PathBuf, bool), String> {
    let cg = std::fs::read_to_string("/proc/self/cgroup").map_err(|e| format!("cannot read /proc/self/cgroup: {e}"))?;
    let mounts = std::fs::read_to_string("/proc/self/mountinfo").map_err(|e| format!("cannot read /proc/self/mountinfo: {e}"))?;
    // mountinfo: id parent major:minor root mountpoint options [optional...] - fstype source superoptions
    let mount = |v2: bool| {
        mounts.lines().find_map(|l| {
            let (pre, post) = l.split_once(" - ")?;
            let pre: Vec<&str> = pre.split(' ').collect();
            let post: Vec<&str> = post.split(' ').collect();
            let ok = if v2 { post.first() == Some(&"cgroup2") } else { post.first() == Some(&"cgroup") && post.get(2).is_some_and(|o| o.split(',').any(|x| x == "pids")) };
            if ok && pre.len() > 4 { Some((pre[3].to_string(), pre[4].to_string())) } else { None }
        })
    };
    let at = |root: &str, point: &str, path: &str| {
        let rel = if root != "/" { path.strip_prefix(root).unwrap_or(path) } else { path };
        PathBuf::from(point).join(rel.trim_start_matches('/'))
    };
    for line in cg.lines() {
        let mut f = line.splitn(3, ':');
        let (_, ctl, path) = (f.next(), f.next().unwrap_or(""), f.next().unwrap_or(""));
        if ctl.split(',').any(|c| c == "pids") {
            if let Some((root, point)) = mount(false) {
                return Ok((at(&root, &point, path), false));
            }
        }
    }
    for line in cg.lines() {
        if let Some(path) = line.strip_prefix("0::") {
            if let Some((root, point)) = mount(true) {
                return Ok((at(&root, &point, path), true));
            }
        }
    }
    Err("no cgroup pids controller is mounted".into())
}

pub struct Group {
    dir: PathBuf,
    procs: std::fs::File,
}

impl Group {
    // A new cgroup under this process's own, holding at most `n` threads and processes together.
    pub fn new(n: u64) -> Result<Group, String> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let (base, v2) = own()?;
        if v2 {
            // v2 gives a child the pids controller only when this cgroup hands it down.
            let sub = base.join("cgroup.subtree_control");
            let has = std::fs::read_to_string(&sub).is_ok_and(|t| t.split_whitespace().any(|c| c == "pids"));
            if !has {
                std::fs::write(&sub, "+pids").map_err(|e| format!("cgroup v2 at {} does not hand the pids controller down, and cannot be made to: {e}", base.display()))?;
            }
        }
        let dir = base.join(format!("brevi-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst)));
        std::fs::create_dir(&dir).map_err(|e| format!("cannot create a cgroup in {}: {e}", base.display()))?;
        let g = (|| {
            std::fs::write(dir.join("pids.max"), n.to_string()).map_err(|e| format!("cannot set pids.max in {}: {e}", dir.display()))?;
            let procs = std::fs::OpenOptions::new().write(true).open(dir.join("cgroup.procs")).map_err(|e| format!("cannot open {}: {e}", dir.join("cgroup.procs").display()))?;
            Ok(procs)
        })();
        match g {
            Ok(procs) => Ok(Group { dir, procs }),
            Err(e) => {
                let _ = std::fs::remove_dir(&dir);
                Err(e)
            }
        }
    }

    // The started process moves itself into this cgroup before it runs the program; if it cannot, it never
    // runs (the spawn fails).
    pub fn enter_on_start(&self, c: &mut Command) {
        use std::os::unix::io::AsRawFd;
        use std::os::unix::process::CommandExt;
        let fd = self.procs.as_raw_fd();
        unsafe {
            c.pre_exec(move || {
                // "0": the writing process itself.
                if write(fd, b"0".as_ptr(), 1) != 1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    // Whether the kernel refused a thread or process at the limit.
    pub fn refused(&self) -> bool {
        std::fs::read_to_string(self.dir.join("pids.events")).is_ok_and(|t| t.lines().any(|l| l.strip_prefix("max ").and_then(|n| n.trim().parse::<u64>().ok()).is_some_and(|n| n > 0)))
    }
}

impl Drop for Group {
    // Anything still in the cgroup is killed, then the cgroup is removed.
    fn drop(&mut self) {
        let _ = std::fs::write(self.dir.join("cgroup.kill"), "1");
        for _ in 0..400 {
            let left = std::fs::read_to_string(self.dir.join("cgroup.procs")).unwrap_or_default();
            let pids: Vec<i32> = left.split_whitespace().filter_map(|p| p.parse().ok()).collect();
            if pids.is_empty() && std::fs::remove_dir(&self.dir).is_ok() {
                return;
            }
            for p in pids {
                unsafe {
                    kill(p, 9);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let _ = std::fs::remove_dir(&self.dir);
    }
}

// Whether a threads limit can be held here (a cgroup is created and removed once, then remembered).
pub fn available() -> Result<(), String> {
    static OK: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
    OK.get_or_init(|| Group::new(1).map(drop)).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    // The kernel refuses the process past the limit, and the cgroup goes with what is left in it.
    #[test]
    fn holds_and_cleans_up() {
        if available().is_err() {
            return;
        }
        let g = Group::new(2).unwrap();
        let dir = g.dir.clone();
        let mut c = Command::new("sh");
        c.args(["-c", "sleep 5 & sleep 5 & sleep 5 & wait"]);
        g.enter_on_start(&mut c);
        let mut child = c.spawn().unwrap();
        let _ = child.wait();
        assert!(g.refused());
        let mut left = Command::new("sleep");
        left.arg("30");
        g.enter_on_start(&mut left);
        let mut held = left.spawn().unwrap();
        drop(g);
        assert!(!dir.exists());
        let _ = held.wait();
    }
}
