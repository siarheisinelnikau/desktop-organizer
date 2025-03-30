fn main() {
    println!("cargo:rerun-if-changed=assets/app.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("assets/app.manifest");
        println!("cargo:rustc-link-arg-bin=DesktopOrganizer=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=DesktopOrganizer=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
}
