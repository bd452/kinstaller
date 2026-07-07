fn main() {
    println!("cargo:rerun-if-changed=shim/kpmio_shim.c");
    cc::Build::new()
        .file("shim/kpmio_shim.c")
        .warnings(true)
        .compile("kinstaller_kpm_shim");
}
