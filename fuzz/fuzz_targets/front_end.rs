// .bv and Python source through the typed build's front end (parse, records, inference, the typed emitter):
// any text must give Rust or a diagnostic, never a panic, a hang or a crash.
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|src: &str| {
    brevi_fuzz::api::front_end(src);
});
