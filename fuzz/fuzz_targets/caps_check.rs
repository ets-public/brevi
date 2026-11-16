// The sandbox's build-time capability check (src/caps.rs check_src) on any source: it must answer (refuse or
// pass), never panic, since a panic there would be a way around the check.
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|src: &str| {
    brevi_fuzz::api::caps_check(src);
});
