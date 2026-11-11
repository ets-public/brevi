// ---------- pinned engine libraries (dynpy/libpin.rs) ----------
// DuckDB, ONNX Runtime and (on Windows) OpenSSL are native libraries found when the program is built, in a
// Python package or Python's own folder. Their SHA-256 is taken then and pasted into the program (dynpy/mod.rs
// pins()), and the program loads a library only if its bytes still hash to that, so a file swapped after the
// build (by another program granted write to that folder, an earlier sandboxed run, a planted copy) never
// runs. The check holds for the bytes that are mapped, not just the file that was read:
//   - Linux: the bytes are read once and hashed, written to a memfd only this process holds, sealed (no write,
//     grow or shrink, ever), and the memfd itself is loaded (/proc/self/fd/N): what is loaded is what was checked.
//   - macOS: the bytes are read once, hashed, and written to a new file in a new private folder (0700, the file
//     made read-only before the check), which is loaded and then removed.
//   - Windows: the file is opened with write and delete sharing denied, and so is every folder on its resolved
//     path, so while it is checked and loaded no one can write, rename or replace it or any folder above it.
//     It is loaded by that resolved path with LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | USER_DIRS | SYSTEM32: its own
//     dependencies come from its own folder, System32 or a folder the build found (CPython's, add_dir), never
//     the program's folder, the working folder or PATH.
// Also compiled into the compiler (dynpy/mod.rs), which uses sha256 to take the pins.
#[allow(dead_code, non_snake_case)]
mod brevi_libpin {
    use std::ffi::c_void;
    pub type P = *mut c_void;

    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    fn blocks_soft(h: &mut [u32; 8], data: &[u8]) {
        for ch in data.chunks_exact(64) {
            let mut w = [0u32; 64];
            for i in 0..16 {
                w[i] = u32::from_be_bytes([ch[4 * i], ch[4 * i + 1], ch[4 * i + 2], ch[4 * i + 3]]);
            }
            for i in 16..64 {
                let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
                let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
            }
            let mut v = *h;
            for i in 0..64 {
                let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
                let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
                let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
                let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
                let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
                v = [t1.wrapping_add(s0.wrapping_add(maj)), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
            }
            for (x, y) in h.iter_mut().zip(v) {
                *x = x.wrapping_add(y);
            }
        }
    }

    // The x86-64 SHA extensions (most CPUs since 2017): about ten times the plain code, so checking a 30 MB
    // library costs milliseconds.
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "sha,sse2,ssse3,sse4.1")]
    unsafe fn blocks_ni(h: &mut [u32; 8], data: &[u8]) {
        unsafe {
            use std::arch::x86_64::*;
            let mask = _mm_set_epi64x(0x0c0d_0e0f_0809_0a0bu64 as i64, 0x0405_0607_0001_0203u64 as i64);
            let abcd = _mm_shuffle_epi32(_mm_loadu_si128(h.as_ptr() as *const __m128i), 0xB1);
            let efgh = _mm_shuffle_epi32(_mm_loadu_si128(h.as_ptr().add(4) as *const __m128i), 0x1B);
            let mut abef = _mm_alignr_epi8(abcd, efgh, 8);
            let mut cdgh = _mm_blend_epi16(efgh, abcd, 0xF0);
            for ch in data.chunks_exact(64) {
                let (abef0, cdgh0) = (abef, cdgh);
                let p = ch.as_ptr() as *const __m128i;
                let mut w = [
                    _mm_shuffle_epi8(_mm_loadu_si128(p), mask),
                    _mm_shuffle_epi8(_mm_loadu_si128(p.add(1)), mask),
                    _mm_shuffle_epi8(_mm_loadu_si128(p.add(2)), mask),
                    _mm_shuffle_epi8(_mm_loadu_si128(p.add(3)), mask),
                ];
                for i in 0..16 {
                    if i >= 4 {
                        let t = _mm_add_epi32(_mm_sha256msg1_epu32(w[0], w[1]), _mm_alignr_epi8(w[3], w[2], 4));
                        let n = _mm_sha256msg2_epu32(t, w[3]);
                        w = [w[1], w[2], w[3], n];
                    }
                    let x = if i < 4 { w[i] } else { w[3] };
                    let t1 = _mm_add_epi32(x, _mm_loadu_si128(K.as_ptr().add(4 * i) as *const __m128i));
                    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, t1);
                    abef = _mm_sha256rnds2_epu32(abef, cdgh, _mm_shuffle_epi32(t1, 0x0E));
                }
                abef = _mm_add_epi32(abef, abef0);
                cdgh = _mm_add_epi32(cdgh, cdgh0);
            }
            let feba = _mm_shuffle_epi32(abef, 0x1B);
            let dchg = _mm_shuffle_epi32(cdgh, 0xB1);
            _mm_storeu_si128(h.as_mut_ptr() as *mut __m128i, _mm_blend_epi16(feba, dchg, 0xF0));
            _mm_storeu_si128(h.as_mut_ptr().add(4) as *mut __m128i, _mm_alignr_epi8(dchg, feba, 8));
        }
    }

    fn blocks(h: &mut [u32; 8], data: &[u8]) {
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("sha") && std::is_x86_feature_detected!("sse4.1") && std::is_x86_feature_detected!("ssse3") {
            return unsafe { blocks_ni(h, data) };
        }
        blocks_soft(h, data)
    }

    pub fn sha256(data: &[u8]) -> [u8; 32] {
        let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
        let full = data.len() / 64 * 64;
        blocks(&mut h, &data[..full]);
        let mut tail = data[full..].to_vec();
        tail.push(0x80);
        while tail.len() % 64 != 56 {
            tail.push(0);
        }
        tail.extend(((data.len() as u64) * 8).to_be_bytes());
        blocks(&mut h, &tail);
        let mut out = [0u8; 32];
        for (i, x) in h.iter().enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&x.to_be_bytes());
        }
        out
    }

    pub fn sha256_hex(data: &[u8]) -> String {
        sha256(data).iter().map(|b| format!("{b:02x}")).collect()
    }

    fn changed(path: &str) -> String {
        format!("brevi: {path} is not the library this program was built with (its SHA-256 changed since the build, for example by an upgrade); build the program again")
    }

    // Loads `path` if its SHA-256 is `want` (hex). `flag` is dlopen's (ignored on Windows).
    #[cfg(target_os = "linux")]
    pub unsafe fn open(path: &str, want: &str, flag: i32) -> Result<P, String> {
        unsafe {
            use std::ffi::{c_char, c_int, c_uint, CString};
            use std::io::{Read, Write};
            use std::os::unix::io::FromRawFd;
            unsafe extern "C" {
                fn memfd_create(name: *const c_char, flags: c_uint) -> c_int;
                fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
                fn dlopen(f: *const c_char, flag: c_int) -> P;
                fn dlerror() -> *const c_char;
            }
            let mut bytes = vec![];
            std::fs::File::open(path).and_then(|mut f| f.read_to_end(&mut bytes)).map_err(|e| format!("brevi: cannot read {path}: {e}"))?;
            // Checked as read; then written to a memfd only this process holds, sealed, and that memfd is what is
            // loaded, so what is loaded is what was checked.
            if sha256_hex(&bytes) != want {
                return Err(changed(path));
            }
            // MFD_CLOEXEC | MFD_ALLOW_SEALING, and MFD_EXEC where the kernel knows it (6.3+, which may refuse
            // executable memfds made without it).
            let mut fd = memfd_create(c"brevi-lib".as_ptr(), 0x1 | 0x2 | 0x10);
            if fd < 0 {
                fd = memfd_create(c"brevi-lib".as_ptr(), 0x1 | 0x2);
            }
            if fd < 0 {
                return Err(format!("brevi: cannot load {path}: no memfd ({})", std::io::Error::last_os_error()));
            }
            let mut f = std::fs::File::from_raw_fd(fd);
            f.write_all(&bytes).map_err(|e| format!("brevi: cannot load {path}: {e}"))?;
            drop(bytes);
            // F_ADD_SEALS: F_SEAL_SEAL | F_SEAL_SHRINK | F_SEAL_GROW | F_SEAL_WRITE. After this the bytes cannot change.
            if fcntl(fd, 1033, 1 | 2 | 4 | 8) != 0 {
                return Err(format!("brevi: cannot load {path}: the copy could not be sealed ({})", std::io::Error::last_os_error()));
            }
            let c = CString::new(format!("/proc/self/fd/{fd}")).unwrap();
            let h = dlopen(c.as_ptr(), flag);
            if h.is_null() {
                let e = dlerror();
                let why = if e.is_null() { String::new() } else { std::ffi::CStr::from_ptr(e).to_string_lossy().into_owned() };
                return Err(format!("brevi: cannot load {path}: {why}"));
            }
            // The memfd stays open for the life of the program: the library's own name for itself is that path.
            std::mem::forget(f);
            Ok(h)
        }
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    pub unsafe fn open(path: &str, want: &str, flag: i32) -> Result<P, String> {
        unsafe {
            use std::ffi::{c_char, c_int, CString};
            use std::io::Write;
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            unsafe extern "C" {
                fn mkdtemp(t: *mut c_char) -> *mut c_char;
                fn dlopen(f: *const c_char, flag: c_int) -> P;
                fn dlerror() -> *const c_char;
            }
            let bytes = std::fs::read(path).map_err(|e| format!("brevi: cannot read {path}: {e}"))?;
            let tmp = std::env::temp_dir().join("brevi-lib-XXXXXX");
            let mut t = CString::new(tmp.to_string_lossy().as_bytes()).unwrap().into_bytes_with_nul();
            if mkdtemp(t.as_mut_ptr() as *mut c_char).is_null() {
                return Err(format!("brevi: cannot load {path}: no private folder ({})", std::io::Error::last_os_error()));
            }
            let dir = std::path::PathBuf::from(String::from_utf8_lossy(&t[..t.len() - 1]).into_owned());
            let name = std::path::Path::new(path).file_name().map(|n| n.to_os_string()).unwrap_or_else(|| "lib".into());
            let file = dir.join(name);
            let res = (|| -> Result<P, String> {
                let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&file).map_err(|e| format!("brevi: cannot load {path}: {e}"))?;
                f.write_all(&bytes).map_err(|e| format!("brevi: cannot load {path}: {e}"))?;
                f.set_permissions(std::fs::Permissions::from_mode(0o400)).map_err(|e| format!("brevi: cannot load {path}: {e}"))?;
                drop(f);
                std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).map_err(|e| format!("brevi: cannot load {path}: {e}"))?;
                // The copy is checked, not the bytes read: those are what gets loaded.
                let copy = std::fs::read(&file).map_err(|e| format!("brevi: cannot load {path}: {e}"))?;
                if sha256_hex(&copy) != want {
                    return Err(changed(path));
                }
                let c = CString::new(file.to_string_lossy().as_bytes()).unwrap();
                let h = dlopen(c.as_ptr(), flag);
                if h.is_null() {
                    let e = dlerror();
                    let why = if e.is_null() { String::new() } else { std::ffi::CStr::from_ptr(e).to_string_lossy().into_owned() };
                    return Err(format!("brevi: cannot load {path}: {why}"));
                }
                Ok(h)
            })();
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).ok();
            std::fs::remove_file(&file).ok();
            std::fs::remove_dir(&dir).ok();
            res
        }
    }

    #[cfg(windows)]
    unsafe extern "system" {
        fn CreateFileW(name: *const u16, access: u32, share: u32, sec: P, disp: u32, flags: u32, tmpl: P) -> P;
        fn ReadFile(h: P, buf: *mut u8, n: u32, read: *mut u32, ov: P) -> i32;
        fn CloseHandle(h: P) -> i32;
        fn GetFinalPathNameByHandleW(h: P, buf: *mut u16, n: u32, flags: u32) -> u32;
        fn LoadLibraryExW(f: *const u16, file: P, flags: u32) -> P;
        fn GetLastError() -> u32;
    }

    #[cfg(windows)]
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    // LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32 (and USER_DIRS, below): a library's dependencies come from its
    // own folder or System32 only. Every Windows load of an engine library uses it (the path must be absolute).
    // LOAD_LIBRARY_SEARCH_USER_DIRS (0x400) adds the folders given to add_dir: CPython's own folder, for an
    // extension module (DuckDB's) that imports python3.dll or the C++ runtime that ships next to python.exe.
    #[cfg(windows)]
    pub const SEARCH: u32 = 0x100 | 0x400 | 0x800;

    #[cfg(windows)]
    unsafe extern "system" {
        fn AddDllDirectory(dir: *const u16) -> P;
    }

    // Lets the libraries this program loads find their dependencies in `dir` (an absolute folder the build
    // found, never the program's or working folder).
    #[cfg(windows)]
    pub unsafe fn add_dir(dir: &str) {
        unsafe {
            if std::path::Path::new(dir).is_absolute() {
                AddDllDirectory(wide(dir).as_ptr());
            }
        }
    }

    #[cfg(unix)]
    pub unsafe fn add_dir(_dir: &str) {}

    // A library named by the caller (BREVI_*_LIB outside the sandbox): not pinned, but still loaded only by an
    // absolute path, with the safe search for its dependencies.
    #[cfg(windows)]
    pub unsafe fn open_unpinned(path: &str) -> Result<P, String> {
        unsafe {
            if !std::path::Path::new(path).is_absolute() {
                return Err(format!("brevi: {path}: a library must be named by its absolute path on Windows"));
            }
            let h = LoadLibraryExW(wide(path).as_ptr(), std::ptr::null_mut(), SEARCH);
            if h.is_null() {
                return Err(format!("brevi: cannot load {path} (Windows error {})", GetLastError()));
            }
            Ok(h)
        }
    }

    #[cfg(unix)]
    pub unsafe fn open_unpinned(path: &str) -> Result<P, String> {
        unsafe {
            unsafe extern "C" {
                fn dlopen(f: *const std::ffi::c_char, flag: i32) -> P;
            }
            let c = std::ffi::CString::new(path).map_err(|_| format!("brevi: cannot load {path}"))?;
            let h = dlopen(c.as_ptr(), 2);
            if h.is_null() { Err(format!("brevi: cannot load {path}")) } else { Ok(h) }
        }
    }

    #[cfg(windows)]
    pub unsafe fn open(path: &str, want: &str, _flag: i32) -> Result<P, String> {
        unsafe {
            const GENERIC_READ: u32 = 0x8000_0000;
            const FILE_LIST_DIRECTORY: u32 = 1;
            const SHARE_READ: u32 = 1;
            const SHARE_WRITE: u32 = 2;
            const OPEN_EXISTING: u32 = 3;
            const BACKUP_SEMANTICS: u32 = 0x0200_0000;
            let invalid = usize::MAX as P;
            if !std::path::Path::new(path).is_absolute() {
                return Err(format!("brevi: {path}: a library must be named by its absolute path on Windows"));
            }
            // Read sharing only: while this handle is open nobody can write, rename or delete the file, and an
            // open by someone already writing it fails here.
            let fh = CreateFileW(wide(path).as_ptr(), GENERIC_READ, SHARE_READ, std::ptr::null_mut(), OPEN_EXISTING, 0, std::ptr::null_mut());
            if fh.is_null() || fh == invalid {
                return Err(format!("brevi: cannot open {path} (Windows error {})", GetLastError()));
            }
            let mut held = vec![fh];
            let res = (|| -> Result<P, String> {
                // Its real path, links and junctions resolved: that is what is loaded and what is held.
                let mut buf = vec![0u16; 32768];
                let n = GetFinalPathNameByHandleW(fh, buf.as_mut_ptr(), buf.len() as u32, 0);
                if n == 0 || n as usize >= buf.len() {
                    return Err(format!("brevi: cannot resolve {path} (Windows error {})", GetLastError()));
                }
                let fin = String::from_utf16_lossy(&buf[..n as usize]);
                // Every folder above it, opened without delete sharing: none can be renamed or replaced meanwhile.
                let mut d = std::path::Path::new(&fin).parent();
                while let Some(dir) = d {
                    let s = dir.to_string_lossy();
                    if s.len() <= 7 {
                        break; // \\?\C:\ : a drive root cannot be renamed
                    }
                    let h = CreateFileW(wide(&s).as_ptr(), FILE_LIST_DIRECTORY, SHARE_READ | SHARE_WRITE, std::ptr::null_mut(), OPEN_EXISTING, BACKUP_SEMANTICS, std::ptr::null_mut());
                    if h.is_null() || h == invalid {
                        return Err(format!("brevi: cannot hold the folder {s} of {path} (Windows error {})", GetLastError()));
                    }
                    held.push(h);
                    d = dir.parent();
                }
                let mut bytes: Vec<u8> = Vec::new();
                let mut chunk = vec![0u8; 1 << 20];
                loop {
                    let mut got = 0u32;
                    if ReadFile(fh, chunk.as_mut_ptr(), chunk.len() as u32, &mut got, std::ptr::null_mut()) == 0 {
                        return Err(format!("brevi: cannot read {path} (Windows error {})", GetLastError()));
                    }
                    if got == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&chunk[..got as usize]);
                }
                if sha256_hex(&bytes) != want {
                    return Err(changed(path));
                }
                // \\?\C:\... loads as C:\...; a \\?\UNC\ path keeps its prefix.
                let load = fin.strip_prefix("\\\\?\\").filter(|r| !r.starts_with("UNC\\")).unwrap_or(&fin).to_string();
                let h = LoadLibraryExW(wide(&load).as_ptr(), std::ptr::null_mut(), SEARCH);
                if h.is_null() {
                    return Err(format!("brevi: cannot load {path} (Windows error {})", GetLastError()));
                }
                Ok(h)
            })();
            for h in held {
                CloseHandle(h);
            }
            res
        }
    }
}
