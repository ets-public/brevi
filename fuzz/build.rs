// Cuts the bundle reader (ax_pyb_unpack, AxInf, AxHuff, ax_inflate) out of src/pyrt_load.rs, which Brevi pastes into
// programs as text, so fuzz targets run the same code. Fails loudly if the anchors move.
fn main() {
    let src = std::fs::read_to_string("../src/pyrt_load.rs").expect("src/pyrt_load.rs");
    let from = src.find("// `room`: the longest path").expect("anchor: ax_pyb_unpack comment");
    let to = src.find("pub type AxPybHold").expect("anchor: AxPybHold");
    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("bundle.rs");
    let code = src[from..to].replace("\nfn ax_", "\npub fn ax_");
    std::fs::write(out, format!("#[allow(dead_code)]\n{code}")).unwrap();
    println!("cargo:rerun-if-changed=../src/pyrt_load.rs");
}
