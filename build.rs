fn main() {
    slint_build::compile("ui/app-window.slint").expect("failed to compile Slint UI");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("PROFILE").as_deref() == Ok("release")
    {
        winresource::WindowsResource::new()
            .set_manifest_file("assets/keycrash.manifest")
            .compile()
            .expect("failed to embed the KeyCrash UAC manifest");
    }
}
