//! Acceptance harness using the same native guardian and Rust coordinator as the UI.
#![allow(dead_code)]
#[cfg(target_os = "macos")]
#[path = "../src/macos/detector.rs"]
mod detector;
#[cfg(target_os = "macos")]
#[path = "../src/macos/filters.rs"]
mod filters;
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
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    if let Some(code) = filters::guardian_entry() {
        std::process::exit(code);
    }
    unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let _: objc2::runtime::Bool = msg_send![app,setActivationPolicy:1isize];
        let _: () = msg_send![app, finishLaunching];
    }
    let args: Vec<_> = std::env::args().collect();
    let all = filters::candidates().unwrap();
    if args[1] == "list" {
        println!(
            "{}",
            serde_json::json!({"candidates":all.iter().map(|c|serde_json::json!({"pid":c.pid,"name":c.name,"path":c.path})).collect::<Vec<_>>()})
        );
        return;
    }
    assert_eq!(
        args.len(),
        5,
        "locate|cancel|cancel-after key modifiers pid,pid"
    );
    let pids: Vec<u32> = args[4].split(',').map(|s| s.parse().unwrap()).collect();
    let selected: Vec<_> = all.into_iter().filter(|c| pids.contains(&c.pid)).collect();
    assert_eq!(
        selected.len(),
        pids.len(),
        "Requested fixture must be an eligible candidate"
    );
    let flags: u32 = args[3].parse().unwrap();
    let shortcut = detector::Shortcut::from_native(
        flags & 4096 != 0,
        flags & 2048 != 0,
        flags & 512 != 0,
        flags & 256 != 0,
        args[2].parse().unwrap(),
        "",
    );
    let cancel = Arc::new(AtomicBool::new(args[1] == "cancel"));
    if args[1] == "cancel-after" {
        let cancel = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1800));
            cancel.store(true, Ordering::Relaxed);
        });
    } else {
        assert!(args[1] == "locate" || args[1] == "cancel");
    }
    let result = std::thread::spawn(move || filters::locate(&shortcut, &selected, &cancel))
        .join()
        .unwrap();
    let value = match result {
        Ok(f) => {
            serde_json::json!({"filters":f.filters.iter().map(|o|serde_json::json!({"pid":o.pid,"path":o.path})).collect::<Vec<_>>(),
            "receivers":f.receivers.iter().map(|o|serde_json::json!({"pid":o.pid,"path":o.path})).collect::<Vec<_>>()})
        }
        Err(e) => serde_json::json!({"error":e}),
    };
    println!("{value}");
}
#[cfg(not(target_os = "macos"))]
fn main() {}
