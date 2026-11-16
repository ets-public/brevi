// MCP JSON-RPC messages: the strict JSON reader (src/json.rs), its writer, the request envelope check and every
// tool's argument check (src/mcp.rs validate). Properties: no panic; what the reader accepts, the writer writes
// back as JSON the reader accepts and writes the same way again.
#![no_main]
use brevi_fuzz::api::*;
use libfuzzer_sys::fuzz_target;
const TOOLS: [&str; 9] = ["guide", "check", "run", "test", "patch", "edit", "verify", "show", "nope"];
fuzz_target!(|src: &str| {
    let Some(j) = json_parse(src) else { return };
    let once = json_to_string(&j);
    let again = json_parse(&once).expect("the writer wrote JSON the reader refuses");
    assert_eq!(once, json_to_string(&again), "writing is not stable");
    let _ = mcp_envelope(&j);
    for t in TOOLS {
        let _ = mcp_validate(t, &j);
        if let Some(a) = j.get("arguments") {
            let _ = mcp_validate(t, a);
        }
    }
});
