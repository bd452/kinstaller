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
    slint_build::compile("ui/app.slint").expect("failed to compile Slint UI");
}
