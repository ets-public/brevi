// Starting the programs Brevi builds, and stopping whole process trees (used by `brevi run` and `brevi mcp`).
use std::process::{Child, Command, ExitStatus};

#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn setrlimit(resource: i32, rlim: *const [u64; 2]) -> i32;
    fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
}

// Linux: the child is killed when the process that started it dies, so a program cannot outlive `brevi run`
// or the MCP server (even when they are killed without warning).
pub fn die_with_parent(c: &mut Command) {
    #[cfg(target_os = "linux")]
    unsafe {
        use std::os::unix::process::CommandExt;
        c.pre_exec(|| {
            prctl(1, 9, 0, 0, 0); // PR_SET_PDEATHSIG, SIGKILL
            Ok(())
        });
    }
    #[cfg(not(target_os = "linux"))]
    let _ = c;
}

// A command for a built program (or the python3 fallback). With BREVI_MEM_MB set (Linux), the program gets at
// most that much address space, so a runaway allocation stops with an error instead of taking the machine.
pub fn program(bin: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut c = Command::new(bin);
    die_with_parent(&mut c);
    #[cfg(target_os = "linux")]
    if let Some(mb) = std::env::var("BREVI_MEM_MB").ok().and_then(|v| v.trim().parse::<u64>().ok()).filter(|m| *m > 0) {
        use std::os::unix::process::CommandExt;
        let bytes = mb.saturating_mul(1 << 20);
        // RLIMIT_AS is 9 on every Linux architecture Rust targets.
        unsafe {
            c.pre_exec(move || {
                setrlimit(9, &[bytes, bytes]);
                Ok(())
            });
        }
    }
    c
}

// The exit code to pass on for a program's status. A program killed by a signal has no code: say what
// happened on stderr (one JSON line, like other runtime errors) and use 128 + the signal, as shells do.
pub fn code(status: ExitStatus) -> i32 {
    if let Some(c) = status.code() {
        return c;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            let msg = match sig {
                9 => "the program was killed (signal 9): out of memory, or stopped from outside".to_string(),
                11 | 7 => format!("the program crashed (signal {sig}): often recursion too deep for the stack"),
                6 => "the program aborted (signal 6)".to_string(),
                s => format!("the program was stopped by signal {s}"),
            };
            eprintln!("{{\"e\":\"signal\",\"msg\":\"{msg}\"}}");
            return 128 + sig;
        }
    }
    1
}

// Puts the child in its own process group, so `stop` reaches everything it starts (rustc, the built
// program, python3).
pub fn own_group(c: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x200;
        c.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }
}

// Kills a child started with own_group and everything under it.
pub fn stop(child: &mut Child) {
    stop_pid(child.id());
    let _ = child.kill();
}

pub fn stop_pid(pid: u32) {
    #[cfg(unix)]
    unsafe {
        kill(-(pid as i32), 9);
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

// Whether a process is still running (unix; elsewhere assumed so).
pub fn alive(pid: u32) -> bool {
    #[cfg(unix)]
    unsafe {
        kill(pid as i32, 0) == 0 || std::io::Error::last_os_error().raw_os_error() != Some(3)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}
