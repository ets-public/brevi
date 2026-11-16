// The compiler, built as a library for the fuzz targets: src/main.rs is pasted in whole, so every module and
// `crate::` path resolves as it does in the brevi binary. Nothing here ships.
#![allow(dead_code, unused_imports, unused_variables, unused_mut, unexpected_cfgs)]
include!("../../src/main.rs");

// Code Brevi pastes into the programs it builds rather than compiling into itself, built here so it can be fuzzed.
pub mod pasted {
    // --allow-read DIR / --allow-write DIR: the run-time path check (src/fsscope.rs).
    pub mod fsscope {
        include!("../../src/fsscope.rs");
    }
    // The standalone-CPython bundle reader (src/pyrt_load.rs, ax_pyb_unpack and ax_inflate), cut out by build.rs.
    pub mod bundle {
        include!(concat!(env!("OUT_DIR"), "/bundle.rs"));
    }
}

// Entry points for fuzz_targets/: thin wrappers over crate-private functions.
pub mod api {
    pub use crate::json::{parse as json_parse, J};
    pub fn json_to_string(j: &J) -> String {
        crate::mcp::to_json(j)
    }
    pub fn mcp_validate(tool: &str, a: &J) -> Result<J, String> {
        crate::mcp::validate(tool, a)
    }
    pub fn mcp_envelope(params: &J) -> Result<bool, String> {
        crate::mcp::envelope(params)
    }
    pub fn mcp_within(ceiling: &[String], g: &str) -> bool {
        crate::mcp::within(ceiling, g)
    }
    pub fn mcp_grants(name: &str, a: &J, trust: bool, ceiling: &[String]) -> Result<(bool, Vec<String>), String> {
        crate::mcp::grants(name, a, trust, ceiling)
    }
    pub fn http_read(bytes: &[u8]) -> Option<(String, String, Vec<(String, String)>, Vec<u8>, bool)> {
        let mut r = std::io::BufReader::new(bytes);
        match crate::mcp_http::read_req(&mut r) {
            crate::mcp_http::Read_::Req(q) => Some((q.method, q.path, q.headers, q.body, q.close)),
            _ => None,
        }
    }
    pub type Limits = crate::proc::Limits;
    pub fn limit_flags(argv: &mut Vec<String>) -> Result<Limits, String> {
        crate::proc::take_limit_flags(argv)
    }
    pub fn lex_bv(src: &str) -> bool {
        crate::lexer::lex(src).is_ok()
    }
    pub fn parse_bv(src: &str) -> bool {
        crate::parser::parse_program(src).is_ok()
    }
    pub fn parse_py(src: &str) -> bool {
        crate::dynpy::parse::parse(src).is_ok()
    }
    pub fn parse_fstring(body: &str, raw: bool) -> bool {
        crate::dynpy::parse::parse_fstring(body, raw, 1).is_ok()
    }
    pub fn caps_check(src: &str) -> usize {
        crate::caps::check_src(src, &[]).len()
    }
    pub fn parse_net(v: &str) -> Result<Vec<String>, String> {
        crate::caps::parse_net(v)
    }
    pub fn parse_grant(py: bool, v: &str) -> Result<Vec<String>, String> {
        crate::caps::parse_grant(py, v)
    }
}
