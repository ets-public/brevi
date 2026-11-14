// ---------- sys.modules at startup ----------
// What CPython 3.13 has there before a script runs, shared by the compiler (emit.rs, which
// decides at build time which reads are exact) and the runtime (rt_mod.rs, for keys only known when it runs).
// As CPython 3.13 has it when `python3.13 script.py` starts (site on; no .pth file, sitecustomize or
// PYTHON* variable adds to it), measured on Linux and read from CPython's Lib for Windows. A name starting with an
// underscore, encodings and its codecs and the modules an environment may add are left out: no answer is sure
// for them (sysmod_unsure).
pub const SYSMOD_STARTUP: &[&str] = &["__main__", "abc", "builtins", "codecs", "genericpath", "io", "marshal", "os", "os.path", "site", "stat", "sys", "time", "zipimport"];
#[cfg(not(windows))]
pub const SYSMOD_STARTUP_OS: &[&str] = &["posix", "posixpath", "errno"];
#[cfg(windows)]
pub const SYSMOD_STARTUP_OS: &[&str] = &["nt", "ntpath"];
// in sys.modules at startup in some environments only, or imported by builtins and the parser without an import
// statement (`x.__reduce_ex__` imports copyreg, `\N{...}` and the namereplace handler unicodedata)
pub fn sysmod_unsure(k: &str) -> bool {
    (k.starts_with('_') && k != "__main__")
        || k == "encodings"
        || k.starts_with("encodings.")
        || matches!(k, "__future__" | "sitecustomize" | "usercustomize" | "linecache" | "warnings" | "locale" | "copyreg" | "unicodedata" | "readline" | "rlcompleter" | "winreg" | "errno" | "posix" | "posixpath" | "nt" | "ntpath")
            && !SYSMOD_STARTUP_OS.contains(&k)
}
pub fn sysmod_startup(k: &str) -> bool {
    SYSMOD_STARTUP.contains(&k) || SYSMOD_STARTUP_OS.contains(&k)
}
