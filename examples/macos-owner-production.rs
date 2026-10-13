//! Acceptance harness for the actual product backend, launched via a trusted
//! GUI test host. Input injection requires explicit `locate` mode.
#![allow(dead_code)]
#[cfg(target_os = "macos")]
#[path = "../src/macos/detector.rs"]
mod detector;
#[cfg(target_os = "macos")]
#[path = "../src/macos/keys.rs"]
mod mac_keys;
#[cfg(target_os = "macos")]
#[path = "../src/macos/owner_probe.rs"]
mod owner_probe;
#[cfg(target_os = "macos")]
#[path = "../src/report.rs"]
mod report;

#[cfg(target_os = "macos")]
fn main() {
    use objc2::{class, msg_send, runtime::AnyObject};
    use std::sync::atomic::AtomicBool;
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4, "mode key-code Carbon-modifiers");
    let key: u16 = args[2].parse().unwrap();
    let flags: u32 = args[3].parse().unwrap();
    let shortcut = detector::Shortcut::from_native(
        flags & 4096 != 0,
        flags & 2048 != 0,
        flags & 512 != 0,
        flags & 256 != 0,
        key,
        "",
    );
    unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, finishLaunching];
    }
    if args[1] == "check" {
        println!("{:?}", detector::probe_shortcut(&shortcut));
        return;
    }
    assert!(args[1] == "locate" || args[1] == "cancel");
    let cancel = args[1] == "cancel";
    let result =
        std::thread::spawn(move || owner_probe::locate_owner(&shortcut, &AtomicBool::new(cancel)))
            .join()
            .unwrap();
    match result {
        owner_probe::OwnerProbeResult::Found(owners) => {
            for owner in owners {
                println!("FOUND\t{}\t{}", owner.pid, owner.path);
            }
        }
        other => println!("{other:?}"),
    }
}
#[cfg(not(target_os = "macos"))]
fn main() {
    panic!("Requires macOS");
}
