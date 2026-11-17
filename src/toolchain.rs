// Brevi's pinned toolchain: Rust and a linker, fetched once when the machine has none, so Brevi needs no Rust
// install. `rustc_cmd()` (src/main.rs) asks `rustc()` here for the compiler and `linker_args()` for the linker.
//
// Rust: the rustc on PATH when there is one (BREVI_TOOLCHAIN=auto, the default). With none, the Rust release CI
// tests with (.github/rust-version), downloaded from static.rust-lang.org. Every file is checked against the
// SHA-256 compiled into brevi (src/toolchain_pins.rs, written by tools/toolchain_pins.py), and a download that
// does not match is never unpacked. BREVI_TOOLCHAIN=system never downloads; BREVI_TOOLCHAIN=pinned always uses
// the pinned one.
//
// Linker: rustc links through a C compiler (`cc`), which needs the C library's development files (Linux) or
// Apple's Command Line Tools (macOS). When those are missing, and always with the pinned toolchain on Windows
// (whose usual linker comes with Visual Studio), zig links instead: rustc is told to link with `brevi-cc`, a copy
// of brevi kept beside zig that runs `zig cc` for this machine (cc_main). BREVI_LINKER=cc or =zig picks one.
//
// Platforms: a host is in VERIFIED once the pinned toolchain has passed CI's checks there
// (.github/workflows/pinned-toolchain.yml). Elsewhere the automatic fetch is refused with the reason, and
// BREVI_TOOLCHAIN=pinned (an explicit choice) is still allowed, with a warning.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

include!("toolchain_pins.rs");

// Hosts where the pinned toolchain (with zig as the linker) passed pinned-toolchain.yml.
// macOS Intel was checked as the x86-64 brevi under Rosetta on an Apple silicon runner (as release.yml builds it).
const VERIFIED: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
];

const EXE: &str = std::env::consts::EXE_SUFFIX;

// Brevi's own host triple, as in PINS.
pub(crate) fn host() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        _ => "",
    }
}

#[allow(clippy::type_complexity)]
fn pin() -> Option<&'static (&'static str, &'static str, &'static [(&'static str, &'static str, &'static str)], (&'static str, &'static str))> {
    PINS.iter().find(|p| p.0 == host())
}

// Where the pinned toolchain lives: BREVI_TOOLCHAIN_DIR, else %LOCALAPPDATA%\brevi\toolchain on Windows (not the
// temporary folder, which Windows empties), else the cache's `pinned` folder.
pub(crate) fn root() -> PathBuf {
    if let Ok(d) = std::env::var("BREVI_TOOLCHAIN_DIR") {
        return std::path::absolute(&d).unwrap_or_else(|_| PathBuf::from(d));
    }
    if cfg!(windows) {
        if let Ok(d) = std::env::var("LOCALAPPDATA") {
            return Path::new(&d).join("brevi").join("toolchain");
        }
    }
    crate::cache_dir().join("pinned")
}

fn mode() -> String {
    std::env::var("BREVI_TOOLCHAIN").unwrap_or_default().trim().to_lowercase()
}

fn rust_dir() -> Option<PathBuf> {
    pin().map(|p| root().join(format!("rust-{RUST}-{}", p.1)))
}

fn zig_dir() -> PathBuf {
    root().join(format!("zig-{ZIG}"))
}

// Why the pinned toolchain could not be used, for the error that follows ("rustc not found").
static WHY: OnceLock<String> = OnceLock::new();

pub(crate) fn why() -> Option<&'static str> {
    WHY.get().map(|s| s.as_str())
}

fn refuse(msg: String) -> Option<PathBuf> {
    WHY.set(msg).ok();
    None
}

// The pinned rustc to use, fetched now if needed. None means: use the rustc on PATH (`system` is the rustc found
// there, if any). Called once per process, from rustc_cmd.
pub(crate) fn rustc(system: Option<&Path>) -> Option<PathBuf> {
    let mode = mode();
    let wanted = match mode.as_str() {
        "" | "auto" => system.is_none(),
        "pinned" => true,
        "system" => {
            if system.is_none() {
                WHY.set("BREVI_TOOLCHAIN=system and no rustc on PATH: install Rust from https://rustup.rs, or unset BREVI_TOOLCHAIN to let brevi fetch its pinned Rust".into()).ok();
            }
            false
        }
        other => return refuse(format!("BREVI_TOOLCHAIN={other} is not one of auto, system, pinned")),
    };
    if !wanted {
        return None;
    }
    let Some(dir) = rust_dir() else {
        return refuse(format!("brevi has no pinned Rust for this machine ({} {}): install Rust from https://rustup.rs", std::env::consts::OS, std::env::consts::ARCH));
    };
    // zig links on Linux only without a working cc (linker_args), and cannot link for a Cortex-A53 machine.
    if let Some(why) = cortex_a53().filter(|_| std::env::var("BREVI_LINKER").unwrap_or_default().trim() != "cc" && !cc_works()) {
        return refuse(format!("no rustc on PATH, and brevi's pinned toolchain cannot be used here: {why}"));
    }
    if !VERIFIED.contains(&host()) {
        if mode != "pinned" {
            return refuse(format!(
                "no rustc on PATH, and brevi's pinned toolchain is not verified on {} yet: install Rust from https://rustup.rs (or run brevi in the Docker image ghcr.io/ets-public/brevi-sandbox), or set BREVI_TOOLCHAIN=pinned to try the pinned toolchain anyway",
                host()
            ));
        }
        eprintln!("brevi: warning: the pinned toolchain is not verified on {} yet (BREVI_TOOLCHAIN=pinned)", host());
    }
    let rustc = dir.join("bin").join(format!("rustc{EXE}"));
    if rustc.is_file() {
        return Some(rustc);
    }
    match install_rust(&dir) {
        Ok(()) => Some(rustc),
        Err(e) => refuse(format!("could not fetch brevi's pinned Rust {RUST}: {e}. Install Rust from https://rustup.rs instead, or try again")),
    }
}

// `-C linker=...` for rustc, when zig links (see the top of this file). Empty when the system's linker is used.
pub(crate) fn linker_args(pinned: bool) -> Vec<String> {
    static A: OnceLock<Vec<String>> = OnceLock::new();
    A.get_or_init(|| {
        let use_zig = match std::env::var("BREVI_LINKER").unwrap_or_default().trim() {
            "zig" => true,
            "cc" => false,
            _ => if cfg!(windows) { pinned } else { !cc_works() },
        };
        if !use_zig {
            return vec![];
        }
        match zig_linker() {
            Ok(p) => vec!["-C".into(), format!("linker={}", p.display())],
            Err(e) => {
                eprintln!("brevi: no C linker found, and zig could not be fetched to link instead: {e}");
                vec![]
            }
        }
    })
    .clone()
}

// The pinned cargo beside the pinned rustc, with the environment that makes it use both and the linker.
pub(crate) fn cargo_cmd(rustc: &Path) -> Command {
    let pinned = rust_dir().is_some_and(|d| rustc.starts_with(d));
    let mut c = if pinned { Command::new(rustc.with_file_name(format!("cargo{EXE}"))) } else { Command::new("cargo") };
    if pinned {
        c.env("RUSTC", rustc);
    }
    if let Some(l) = linker_args(pinned).get(1).and_then(|a| a.strip_prefix("linker=")) {
        let triple = pin().map_or("", |p| if pinned { p.1 } else { p.0 });
        c.env(format!("CARGO_TARGET_{}_LINKER", triple.to_uppercase().replace('-', "_")), l);
    }
    c
}

// True when `cc` can link a program here: Linux needs the C library's start files (libc6-dev, glibc-devel),
// macOS the Command Line Tools.
fn cc_works() -> bool {
    if cfg!(target_os = "macos") {
        return Command::new("xcode-select")
            .arg("-p")
            .stdin(Stdio::null())
            .output()
            .is_ok_and(|o| o.status.success() && Path::new(String::from_utf8_lossy(&o.stdout).trim()).is_dir());
    }
    Command::new("cc").arg("-print-file-name=crt1.o").stdin(Stdio::null()).output().is_ok_and(|o| {
        let p = PathBuf::from(String::from_utf8_lossy(&o.stdout).trim());
        o.status.success() && p.is_absolute() && p.is_file()
    })
}

// zig, fetched if needed, and `brevi-cc` beside it (a copy of this brevi, refreshed when brevi changes).
fn zig_linker() -> Result<PathBuf, String> {
    let Some(p) = pin() else { return Err(format!("brevi has no pinned zig for {} {}", std::env::consts::OS, std::env::consts::ARCH)) };
    let dir = zig_dir();
    if !dir.join("zig").join(format!("zig{EXE}")).is_file() {
        install_zig(&dir, p.3)?;
    }
    let cc = dir.join(format!("brevi-cc{EXE}"));
    let stamp = dir.join("brevi-cc.version");
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let mine = format!("{} {:?}", env!("CARGO_PKG_VERSION"), std::fs::metadata(&me).and_then(|m| m.modified()).ok());
    if !cc.is_file() || std::fs::read_to_string(&stamp).ok().as_deref() != Some(mine.as_str()) {
        let _lock = lock(&dir.join(".lock"))?;
        if !crate::copy_bin(&me, &cc) {
            return Err(format!("could not copy brevi to {}", cc.display()));
        }
        std::fs::write(&stamp, &mine).ok();
    }
    Ok(cc)
}

// `brevi-cc ARGS...`: the linker rustc runs when zig links. Runs `zig cc -target <this machine> ARGS...`.
pub(crate) fn is_cc() -> bool {
    std::env::args_os().next().and_then(|a| Path::new(&a).file_stem().map(|s| s.to_string_lossy().to_lowercase())).is_some_and(|s| s == "brevi-cc")
}

pub(crate) fn cc_main() -> ! {
    let me = std::env::current_exe().unwrap_or_default();
    let dir = me.parent().unwrap_or(Path::new("."));
    let mut c = Command::new(dir.join("zig").join(format!("zig{EXE}")));
    c.arg("cc").arg("-target").arg(zig_target());
    for a in std::env::args_os().skip(1) {
        match a.to_str().and_then(cc_arg) {
            Some(Ok(Some(r))) => {
                c.arg(r);
            }
            Some(Ok(None)) => {}
            Some(Err(e)) => {
                eprintln!("brevi-cc: {e}");
                std::process::exit(1)
            }
            None => {
                c.arg(a);
            }
        }
    }
    // rustc's Windows targets unwind with LLVM's libunwind, which zig builds when asked; and the shim below.
    if cfg!(windows) {
        c.arg("-lunwind");
        match win_shim(dir) {
            Ok(o) => {
                c.arg(o);
            }
            Err(e) => {
                eprintln!("brevi-cc: {e}");
                std::process::exit(1)
            }
        }
    }
    // zig keeps the C runtime it builds for the target here, beside itself, not in the user's ~/.cache/zig.
    let cache = dir.join("cache");
    c.env("ZIG_GLOBAL_CACHE_DIR", &cache).env("ZIG_LOCAL_CACHE_DIR", &cache);
    match c.status() {
        Ok(s) => std::process::exit(s.code().unwrap_or(1)),
        Err(e) => {
            eprintln!("brevi-cc: could not run zig in {}: {e}", dir.display());
            std::process::exit(1)
        }
    }
}

// How brevi-cc passes one of rustc's linker arguments to zig cc: None keeps it, Ok(None) drops it, Ok(Some(x))
// passes x instead, Err refuses the link with the reason.
fn cc_arg(a: &str) -> Option<Result<Option<&'static str>, String>> {
    match a {
        // GNU ld's optimisation level: zig warns about it and ignores it.
        "-Wl,-O1" => Some(Ok(None)),
        // rustc asks the linker to patch around Cortex-A53 erratum 843419 on ARM64 Linux; zig cannot pass that on
        // to its linker. Programs built here run here, so it is only dropped where no core has the erratum.
        "-Wl,--fix-cortex-a53-843419" => Some(match cortex_a53() {
            None => Ok(None),
            Some(why) => Err(why),
        }),
        // Windows: zig's MinGW libraries are UCRT ones, so its C runtime stands in for msvcrt (as llvm-mingw's
        // libmsvcrt.a does), and synchronization.lib's functions (WaitOnAddress and the rest) live in this one.
        "-lmsvcrt" if cfg!(windows) => Some(Ok(None)),
        "-lsynchronization" if cfg!(windows) => Some(Ok(Some("-lapi-ms-win-core-synch-l1-2-0"))),
        _ => None,
    }
}

// Windows: functions rustc's code calls that zig's MinGW C library lacks. LLVM turns sin(x) and cos(x) of one x
// into one sincos call on the GNU targets, as llvm-mingw's library provides; here it is sin and cos, the same
// values CPython gets. Compiled with -fno-builtin, so the compiler cannot turn the pair back into a sincos call.
const WIN_SHIM: &str = "double sin(double), cos(double);\nfloat sinf(float), cosf(float);\n\
void sincos(double x, double *s, double *c) { *s = sin(x); *c = cos(x); }\n\
void sincosf(float x, float *s, float *c) { *s = sinf(x); *c = cosf(x); }\n";

// The shim's object file beside zig, compiled once (the name changes with its source).
fn win_shim(dir: &Path) -> Result<PathBuf, String> {
    let h = WIN_SHIM.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    let obj = dir.join(format!("brevi-shim-{h:016x}.o"));
    if obj.is_file() {
        return Ok(obj);
    }
    let id = std::process::id();
    let src = dir.join(format!(".brevi-shim-{id}.c"));
    let tmp = dir.join(format!(".brevi-shim-{id}.o"));
    std::fs::write(&src, WIN_SHIM).map_err(|e| format!("{}: {e}", src.display()))?;
    let cache = dir.join("cache");
    let o = Command::new(dir.join("zig").join(format!("zig{EXE}")))
        .args(["cc", "-target", &zig_target(), "-O2", "-fno-builtin", "-c"])
        .arg(&src)
        .arg("-o")
        .arg(&tmp)
        .env("ZIG_GLOBAL_CACHE_DIR", &cache)
        .env("ZIG_LOCAL_CACHE_DIR", &cache)
        .stdin(Stdio::null())
        .output();
    std::fs::remove_file(&src).ok();
    match o {
        Ok(o) if o.status.success() => {}
        Ok(o) => return Err(format!("could not compile its C shim: {}", String::from_utf8_lossy(&o.stderr).trim())),
        Err(e) => return Err(format!("could not run zig: {e}")),
    }
    // Another link may have made it meanwhile: either file is the same.
    if std::fs::rename(&tmp, &obj).is_err() && !obj.is_file() {
        return Err(format!("could not save {}", obj.display()));
    }
    std::fs::remove_file(&tmp).ok();
    Ok(obj)
}

// Why zig cannot link for this machine: one of its cores is an Arm Cortex-A53 (implementer 0x41, part 0xd03, as in
// a Raspberry Pi 3), whose erratum 843419 needs a linker fix zig's linker is not given.
fn cortex_a53() -> Option<String> {
    if !cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        return None;
    }
    let info = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    a53_in(&info).then(|| "this machine has an Arm Cortex-A53 core, and zig cannot apply the linker fix for its erratum 843419: install a C compiler (gcc) or Rust from https://rustup.rs".to_string())
}

fn a53_in(cpuinfo: &str) -> bool {
    let field = |b: &str, k: &str| b.lines().find_map(|l| l.split_once(':').filter(|(n, _)| n.trim() == k).map(|(_, v)| v.trim().to_lowercase()));
    cpuinfo.split("\n\n").any(|b| field(b, "CPU implementer").as_deref() == Some("0x41") && field(b, "CPU part").as_deref() == Some("0xd03"))
}

fn zig_target() -> String {
    let arch = std::env::consts::ARCH;
    if cfg!(target_os = "macos") {
        format!("{arch}-macos")
    } else if cfg!(windows) {
        format!("{arch}-windows-gnu")
    } else {
        // The machine's own glibc, so the program may use everything it has (zig's default is older).
        format!("{arch}-linux-gnu.{}", glibc())
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn glibc() -> String {
    unsafe extern "C" {
        fn gnu_get_libc_version() -> *const std::ffi::c_char;
    }
    // SAFETY: returns a static, NUL-terminated string.
    let v = unsafe { std::ffi::CStr::from_ptr(gnu_get_libc_version()) }.to_string_lossy().to_string();
    // zig takes MAJOR.MINOR.
    v.split('.').take(2).collect::<Vec<_>>().join(".")
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn glibc() -> String {
    "2.28".into()
}

// ---- fetching ----

fn lock(path: &Path) -> Result<std::fs::File, String> {
    let f = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    f.lock().map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(f)
}

// Windows' own curl and tar (System32), not ones from Git or MSYS that cannot read a .zip.
fn tool(name: &str) -> Command {
    if cfg!(windows) {
        if let Ok(r) = std::env::var("SystemRoot") {
            let p = Path::new(&r).join("System32").join(format!("{name}.exe"));
            if p.is_file() {
                return Command::new(p);
            }
        }
    }
    Command::new(name)
}

// Downloads `url` to `to` and checks its SHA-256 against `sha`.
fn fetch(url: &str, sha: &str, to: &Path) -> Result<(), String> {
    let o = tool("curl")
        .args(["-fsSL", "--retry", "3", "--proto", "=https", "--tlsv1.2", "-o"])
        .arg(to)
        .arg(url)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("curl is needed to fetch the toolchain ({e})"))?;
    if !o.status.success() {
        return Err(format!("downloading {url} failed: {}", String::from_utf8_lossy(&o.stderr).trim()));
    }
    let data = std::fs::read(to).map_err(|e| e.to_string())?;
    let got = crate::dynpy::tok::hex(&crate::dynpy::tok::sha256(&data));
    if got != sha {
        std::fs::remove_file(to).ok();
        return Err(format!("{url} has SHA-256 {got}, not the pinned {sha}: not unpacked"));
    }
    Ok(())
}

fn untar(file: &Path, into: &Path) -> Result<(), String> {
    std::fs::create_dir_all(into).map_err(|e| e.to_string())?;
    let name = file.to_string_lossy();
    let flags = if name.ends_with(".gz") { "-xzf" } else if name.ends_with(".xz") { "-xJf" } else { "-xf" };
    let ok = tool("tar").arg(flags).arg(file).arg("-C").arg(into).stdin(Stdio::null()).output().is_ok_and(|o| o.status.success());
    if ok {
        return Ok(());
    }
    // GNU tar unpacks .xz through the `xz` program, which some minimal Linux images lack: Python reads it itself.
    if name.ends_with(".xz") {
        for py in ["python3", "python"] {
            if Command::new(py).args(["-m", "tarfile", "-e"]).arg(file).arg(into).stdin(Stdio::null()).output().is_ok_and(|o| o.status.success()) {
                return Ok(());
            }
        }
    }
    Err(format!("could not unpack {} (tar{})", file.display(), if name.ends_with(".xz") { " with xz, or python3" } else { "" }))
}

// Moves every entry of `from` into `to`, merging folders (Rust's components share bin/ and lib/).
fn merge(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for e in std::fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
        let dest = to.join(e.file_name());
        if e.file_type().is_ok_and(|t| t.is_dir()) && dest.is_dir() {
            merge(&e.path(), &dest)?;
        } else {
            std::fs::rename(e.path(), &dest).map_err(|err| format!("{} -> {}: {err}", e.path().display(), dest.display()))?;
        }
    }
    Ok(())
}

fn note(what: &str, mb: u32, dir: &Path) {
    eprintln!("brevi: fetching {what} (about {mb} MB, once) into {}", dir.display());
}

// Unpacks the pinned Rust components into `dir`: built in a `.tmp-` folder beside it, then renamed into place,
// so a cut-short fetch never leaves a half toolchain. One process fetches at a time.
fn install_rust(dir: &Path) -> Result<(), String> {
    let p = pin().ok_or("no pinned Rust for this machine")?;
    let parent = dir.parent().ok_or("bad toolchain folder")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    let _lock = lock(&parent.join(".lock-rust"))?;
    if dir.join("bin").join(format!("rustc{EXE}")).is_file() {
        return Ok(());
    }
    let why = if mode() == "pinned" { "because BREVI_TOOLCHAIN=pinned" } else { "because no rustc is on PATH" };
    note(&format!("Rust {RUST}, the version Brevi is tested with, {why}"), 260, dir);
    let tmp = parent.join(format!(".tmp-rust-{}", std::process::id()));
    std::fs::remove_dir_all(&tmp).ok();
    let r = (|| {
        std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
        let sys = tmp.join("sysroot");
        for (name, url, sha) in p.2 {
            let file = tmp.join(format!("{name}.tar.gz"));
            fetch(url, sha, &file)?;
            let x = tmp.join(name);
            untar(&file, &x)?;
            std::fs::remove_file(&file).ok();
            // Each archive holds one folder, whose `components` file names the folders to install.
            let top = std::fs::read_dir(&x).map_err(|e| e.to_string())?.flatten().find(|e| e.path().is_dir()).ok_or("empty archive")?.path();
            let comps = std::fs::read_to_string(top.join("components")).map_err(|e| format!("{name}: no components list: {e}"))?;
            for c in comps.lines().map(str::trim).filter(|c| !c.is_empty()) {
                merge(&top.join(c), &sys)?;
            }
            std::fs::remove_dir_all(&x).ok();
        }
        // The installer's own records, not needed to run.
        std::fs::remove_dir_all(sys.join("lib").join("rustlib").join("manifest-rustc")).ok();
        if !sys.join("bin").join(format!("rustc{EXE}")).is_file() {
            return Err("the archives held no rustc".to_string());
        }
        std::fs::rename(&sys, dir).map_err(|e| format!("{}: {e}", dir.display()))
    })();
    std::fs::remove_dir_all(&tmp).ok();
    r
}

fn install_zig(dir: &Path, (url, sha): (&str, &str)) -> Result<(), String> {
    let parent = dir.parent().ok_or("bad toolchain folder")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    let _lock = lock(&parent.join(".lock-zig"))?;
    let zig = dir.join("zig").join(format!("zig{EXE}"));
    if zig.is_file() {
        return Ok(());
    }
    let why = if std::env::var("BREVI_LINKER").unwrap_or_default().trim() == "zig" {
        "because BREVI_LINKER=zig"
    } else if cfg!(windows) {
        "because the pinned Rust links with zig on Windows"
    } else {
        "because no C linker is installed"
    };
    note(&format!("zig {ZIG} to link programs, {why}"), 55, dir);
    let tmp = parent.join(format!(".tmp-zig-{}", std::process::id()));
    std::fs::remove_dir_all(&tmp).ok();
    let r = (|| {
        let ext = if url.ends_with(".zip") { "zip" } else { "tar.xz" };
        let file = tmp.join(format!("zig.{ext}"));
        std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
        fetch(url, sha, &file)?;
        let x = tmp.join("x");
        untar(&file, &x)?;
        let top = std::fs::read_dir(&x).map_err(|e| e.to_string())?.flatten().find(|e| e.path().is_dir()).ok_or("empty archive")?.path();
        let out = tmp.join("out");
        std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        std::fs::rename(&top, out.join("zig")).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            // Python's tarfile keeps the mode bits, tar does too; this makes sure zig can run either way.
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(out.join("zig").join("zig"), std::fs::Permissions::from_mode(0o755)).ok();
        }
        if !out.join("zig").join(format!("zig{EXE}")).is_file() {
            return Err("the archive held no zig".to_string());
        }
        std::fs::rename(&out, dir).map_err(|e| format!("{}: {e}", dir.display()))
    })();
    std::fs::remove_dir_all(&tmp).ok();
    r
}

// `brevi toolchain`: fetches whatever builds here still need (as the first build would), then prints one JSON
// line saying which rustc and linker builds use. For an install script, a Docker layer, or a check.
pub(crate) fn main(argv: &[String]) -> ! {
    if let Some(o) = argv.first().filter(|a| *a != "install") {
        eprintln!("usage: brevi toolchain [install] (not {o})");
        std::process::exit(2)
    }
    let mut c = crate::rustc_cmd();
    let rustc = c.get_program().to_string_lossy().to_string();
    let linker = c.get_args().map(|a| a.to_string_lossy().to_string()).find_map(|a| a.strip_prefix("linker=").map(String::from));
    let version = c.arg("-V").output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let pinned = rust_dir().is_some_and(|d| Path::new(&rustc).starts_with(d));
    let esc = crate::esc;
    match version {
        Some(v) => {
            println!(
                "{{\"rustc\":\"{}\",\"version\":\"{}\",\"pinned\":{pinned},\"linker\":\"{}\",\"dir\":\"{}\"}}",
                esc(&rustc),
                esc(&v),
                esc(linker.as_deref().unwrap_or("cc")),
                esc(&root().to_string_lossy())
            );
            std::process::exit(0)
        }
        None => {
            println!("{{\"e\":\"toolchain\",\"msg\":\"{}\"}}", esc(why().unwrap_or("no rustc found")));
            std::process::exit(1)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn pins_the_rust_ci_tests_with() {
        assert_eq!(super::RUST, include_str!("../.github/rust-version").trim(), "run python3 tools/toolchain_pins.py");
    }

    #[test]
    fn every_host_has_pins() {
        for p in super::PINS {
            assert!(p.2.iter().any(|c| c.0 == "rustc") && p.2.iter().any(|c| c.0 == "rust-std"), "{}", p.0);
            assert!(p.2.iter().chain([&("zig", p.3 .0, p.3 .1)]).all(|c| c.1.starts_with("https://") && c.2.len() == 64), "{}", p.0);
        }
    }

    #[test]
    fn cortex_a53_is_found_in_cpuinfo() {
        let core = |part: &str| format!("processor\t: 0\nCPU implementer\t: 0x41\nCPU part\t: {part}\n");
        assert!(super::a53_in(&format!("{}\n{}", core("0xd08"), core("0xd03"))));
        assert!(!super::a53_in(&core("0xd0c")));
        assert!(!super::a53_in("processor\t: 0\nCPU implementer\t: 0x51\nCPU part\t: 0xd03\n"));
    }

    #[test]
    fn brevi_cc_maps_rustc_linker_arguments() {
        assert_eq!(super::cc_arg("-Wl,-O1"), Some(Ok(None)));
        assert_eq!(super::cc_arg("-lkernel32"), None);
        if cfg!(windows) {
            assert_eq!(super::cc_arg("-lmsvcrt"), Some(Ok(None)));
            assert_eq!(super::cc_arg("-lsynchronization"), Some(Ok(Some("-lapi-ms-win-core-synch-l1-2-0"))));
        } else {
            assert_eq!(super::cc_arg("-lmsvcrt"), None);
        }
    }

    #[test]
    fn verified_hosts_are_pinned() {
        for h in super::VERIFIED {
            assert!(super::PINS.iter().any(|p| p.0 == *h), "{h}");
        }
    }
}
