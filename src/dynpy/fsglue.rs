
// ---------- files, for Brevi's polars and safetensors (pasted into the program; see _brevi_fs in rt_net.rs) ----------
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

    // The path to open: under --allow-read / --allow-write DIR, the real path, if it is inside them.
    fn scoped(p: &str, write: bool) -> Result<String, NetErr> {
        brevi_fs_path(p, write).map_err(|msg| NetErr { kind: "PermissionError", msg })
    }

    fn read(p: &str) -> Result<String, NetErr> {
        if FS_SANDBOX && !FS_READ {
            return Err(refuse("read", p));
        }
        let b = std::fs::read(scoped(p, false)?).map_err(|e| os_err(e, p))?;
        String::from_utf8(b).map_err(|e| NetErr { kind: "ValueError", msg: format!("invalid utf-8 sequence at byte {}", e.utf8_error().valid_up_to()) })
    }

    fn write(p: &str, t: &str, append: bool) -> Result<(), NetErr> {
        if FS_SANDBOX && !FS_WRITE {
            return Err(refuse("write", p));
        }
        let real = scoped(p, true)?;
        brevi_new_file(&real);
        let mut f = std::fs::OpenOptions::new().write(true).create(true).append(append).truncate(!append).open(real).map_err(|e| os_err(e, p))?;
        f.write_all(t.as_bytes()).map_err(|e| os_err(e, p))
    }

    fn read_at(p: &str, off: u64, len: i64) -> Result<Vec<u8>, NetErr> {
        use std::io::{Read, Seek, SeekFrom};
        if FS_SANDBOX && !FS_READ {
            return Err(refuse("read", p));
        }
        let mut f = std::fs::File::open(scoped(p, false)?).map_err(|e| os_err(e, p))?;
        if f.metadata().map(|m| m.is_dir()).unwrap_or(false) {
            return Err(NetErr { kind: "IsADirectoryError", msg: format!("Is a directory (os error 21): {p}") });
        }
        f.seek(SeekFrom::Start(off)).map_err(|e| os_err(e, p))?;
        let mut out = vec![];
        if len < 0 {
            f.read_to_end(&mut out).map_err(|e| os_err(e, p))?;
        } else {
            f.take(len as u64).read_to_end(&mut out).map_err(|e| os_err(e, p))?;
        }
        Ok(out)
    }

    fn size(p: &str) -> Result<u64, NetErr> {
        if FS_SANDBOX && !FS_READ {
            return Err(refuse("read", p));
        }
        std::fs::metadata(scoped(p, false)?).map(|m| m.len()).map_err(|e| os_err(e, p))
    }

    fn write_bytes(p: &str, b: &[u8]) -> Result<(), NetErr> {
        if FS_SANDBOX && !FS_WRITE {
            return Err(refuse("write", p));
        }
        let real = scoped(p, true)?;
        brevi_new_file(&real);
        std::fs::write(real, b).map_err(|e| os_err(e, p))
    }

    pub fn ops() -> FsOps {
        FsOps { read, write, read_at, size, write_bytes }
    }
}
