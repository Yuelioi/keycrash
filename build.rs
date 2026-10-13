fn main() {
    println!("cargo:rerun-if-changed=assets/keycrash.manifest");
    println!("cargo:rerun-if-changed=assets/keycrash-icon.ico");
    slint_build::compile("ui/app-window.slint").expect("failed to compile Slint UI");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/macos/filter_native.m")
            .flag("-fobjc-arc")
            .flag("-mmacosx-version-min=11.0")
            .compile("keycrash_filter");
        println!("cargo:rustc-link-lib=framework=Cocoa");
        println!("cargo:rustc-link-lib=framework=Carbon");
        println!("cargo:rerun-if-changed=src/macos/filter_native.m");
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("PROFILE").as_deref() == Ok("release")
    {
        winresource::WindowsResource::new()
            .set_icon("assets/keycrash-icon.ico")
            .set_manifest_file("assets/keycrash.manifest")
            .compile()
            .expect("failed to embed the KeyCrash icon and UAC manifest");
    }
}
