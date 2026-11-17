// A standalone program's bundled CPython unpacked to its cache folder (src/pyrt_load.rs ax_pyb_unpack): file names
// come from the bundle. Property: nothing is written outside the folder it unpacks into. Sizes in the bundle are
// capped here (the real bundle is the program's own trailer), so the fuzzer explores names, not huge allocations.
#![no_main]
use libfuzzer_sys::fuzz_target;
use std::sync::atomic::{AtomicU64, Ordering};

fuzz_target!(|data: &[u8]| {
    // Keep each record's data length small: rewrite length fields over 64 KiB.
    let mut d = data.to_vec();
    let mut at = 0;
    while at + 15 <= d.len() {
        let plen = u16::from_le_bytes([d[at + 13], d[at + 14]]) as usize;
        let l = at + 15 + plen;
        if l + 8 > d.len() {
            break;
        }
        for f in [l, l + 4] {
            let v = u32::from_le_bytes(d[f..f + 4].try_into().unwrap()) % 65536;
            d[f..f + 4].copy_from_slice(&v.to_le_bytes());
        }
        let dlen = u32::from_le_bytes(d[l + 4..l + 8].try_into().unwrap()) as usize;
        at = l + 8 + dlen;
    }
    static N: AtomicU64 = AtomicU64::new(0);
    let base = std::env::temp_dir().join(format!("brevi-fuzz-unpack-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    let inner = base.join("a").join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    let _ = brevi_fuzz::pasted::bundle::ax_pyb_unpack(&mut &d[..], &inner, usize::MAX);
    let names: Vec<_> = std::fs::read_dir(&base).unwrap().map(|e| e.unwrap().file_name()).collect();
    let names2: Vec<_> = std::fs::read_dir(base.join("a")).unwrap().map(|e| e.unwrap().file_name()).collect();
    std::fs::remove_dir_all(&base).ok();
    assert!(names == ["a"] && names2 == ["inner"], "wrote outside the folder: {names:?} {names2:?}");
});
