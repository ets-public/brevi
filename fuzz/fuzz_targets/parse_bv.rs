// .bv and Python-style source through Brevi's lexer and parser (src/lexer.rs, src/parser.rs): any text must give
// an AST or an error, never a panic, a hang or a crash.
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|src: &str| {
    brevi_fuzz::api::lex_bv(src);
    brevi_fuzz::api::parse_bv(src);
});
