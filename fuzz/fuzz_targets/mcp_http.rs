// `brevi mcp --http`: one HTTP/1.1 request read from raw bytes (src/mcp_http.rs read_req), before any token or
// Origin check, so anyone who can reach the port controls these bytes. Properties: no panic, and a body is never
// larger than the server's 4 MiB cap.
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    if let Some((_, _, headers, body, _)) = brevi_fuzz::api::http_read(data) {
        assert!(body.len() <= 4 * 1024 * 1024, "body of {} bytes", body.len());
        assert!(headers.iter().all(|(k, _)| k.chars().all(|c| !c.is_ascii_uppercase())));
    }
});
