// Python source through the dynamic build's parser (src/dynpy/parse.rs), and f-string bodies on their own.
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|src: &str| {
    brevi_fuzz::api::parse_py(src);
    brevi_fuzz::api::parse_fstring(src, false);
    brevi_fuzz::api::parse_fstring(src, true);
});
