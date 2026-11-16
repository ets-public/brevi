// --limit values (src/proc.rs Limits::parse, take_limit_flags), which reach the program's runner from the command
// line and from MCP calls. Properties: no panic; a spec that parses is written back (spec(), passed to the child
// brevi) as one that parses again to the same set of limits, so no limit is dropped on the way to the program.
#![no_main]
use brevi_fuzz::api::*;
use libfuzzer_sys::fuzz_target;
fuzz_target!(|spec: &str| {
    let mut l = Limits::default();
    if l.parse(spec).is_ok() {
        let mut back = Limits::default();
        back.parse(&l.spec()).unwrap_or_else(|e| panic!("{:?} -> {:?}: {e}", spec, l.spec()));
        let set = |x: &Limits| [x.cpu.is_some(), x.mem.is_some(), x.time.is_some(), x.output.is_some(), x.file.is_some(), x.threads.is_some(), x.fds.is_some(), x.written.is_some(), x.files.is_some(), x.net.is_some()];
        assert_eq!(set(&l), set(&back), "{spec:?} lost a limit");
        assert!(back.cpu >= l.cpu && back.mem >= l.mem && back.file >= l.file);
    }
    let mut argv: Vec<String> = spec.split('\u{1}').map(String::from).collect();
    let _ = limit_flags(&mut argv);
});
