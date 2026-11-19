// The DEFLATE decoder that unpacks a standalone program's bundled CPython (src/pyrt_load.rs ax_inflate) before its
// shared libraries are loaded. Property: no panic, and never more output than the size asked for.
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    let Some((h, d)) = data.split_at_checked(2) else { return };
    let size = u16::from_le_bytes([h[0], h[1]]) as usize * 16;
    if let Some(out) = brevi_fuzz::pasted::bundle::ax_inflate(d, size) {
        assert!(out.len() <= size.max(1 << 20) , "{} bytes for {size}", out.len());
    }
});
