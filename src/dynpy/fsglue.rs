
// ---------- files, for Brevi's polars (pasted into the program; see _brevi_fs in rt_net.rs) ----------
// Whole-file reads and writes, with polars' error messages ("No such file or directory (os error 2): x.csv").
// Under --sandbox, reading needs the read grant and writing the write grant (FS_READ / FS_WRITE).
mod brevi_fsglue {
    use super::*;
    use brevi_dyn::{FsOps, NetErr};
    use std::io::Write;

    fn os_err(e: std::io::Error, p: &str) -> NetErr {
        let kind = match e.kind() {
            std::io::ErrorKind::NotFound => "FileNotFoundError",
            std::io::ErrorKind::PermissionDenied => "PermissionError",
            std::io::ErrorKind::AlreadyExists => "FileExistsError",
            _ if e.raw_os_error() == Some(21) => "IsADirectoryError",
            _ => "OSError",
        };
        // as polars words it: "No such file or directory (os error 2): data.csv"
        let msg = format!("{e}: {p}");
        NetErr { kind, msg }
    }

    fn refuse(cap: &str, p: &str) -> NetErr {
        NetErr { kind: "PermissionError", msg: format!("brevi: --sandbox: {p:?} needs allow {cap}").replace('"', "'") }
    }

    fn read(p: &str) -> Result<String, NetErr> {
        if FS_SANDBOX && !FS_READ {
            return Err(refuse("read", p));
        }
        let b = std::fs::read(p).map_err(|e| os_err(e, p))?;
        String::from_utf8(b).map_err(|e| NetErr { kind: "ValueError", msg: format!("invalid utf-8 sequence at byte {}", e.utf8_error().valid_up_to()) })
    }

    fn write(p: &str, t: &str, append: bool) -> Result<(), NetErr> {
        if FS_SANDBOX && !FS_WRITE {
            return Err(refuse("write", p));
        }
        let mut f = std::fs::OpenOptions::new().write(true).create(true).append(append).truncate(!append).open(p).map_err(|e| os_err(e, p))?;
        f.write_all(t.as_bytes()).map_err(|e| os_err(e, p))
    }

    pub fn ops() -> FsOps {
        FsOps { read, write }
    }
}
