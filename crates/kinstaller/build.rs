fn main() {
    let fonts_dir = std::path::Path::new("fonts");
    for name in ["LiberationSans-Regular.ttf", "LiberationSans-Bold.ttf"] {
        let path = fonts_dir.join(name);
        assert!(
            path.exists(),
            "missing {path:?} — run `./scripts/fetch-fonts.sh` (or `./scripts/setup.sh`) first"
        );
        println!("cargo:rerun-if-changed={}", path.display());
    }

    // kindlepw2's koxtoolchain sysroot ships glibc 2.7 (no getauxval). Rust std still
    // references the symbol at link time; provide a stub that returns 0 (same as a miss).
    if std::env::var("TARGET").as_deref() == Ok("armv7-unknown-linux-gnueabi") {
        cc::Build::new()
            .file("native/getauxval_stub.c")
            .compile("getauxval_stub");
    }

    slint_build::compile("ui/app.slint").expect("failed to compile Slint UI");
}
