//! Run through scripts/test-macos-hotkeys.py in a logged-in macOS session.
#[cfg(target_os = "macos")]
#[path = "../src/macos/carbon.rs"]
mod carbon;

#[cfg(target_os = "macos")]
fn main() {
    use std::io::{self, Write};
    let args: Vec<_> = std::env::args().collect();
    let key: u16 = args[2].parse().expect("decimal key code");
    let modifiers: u32 = args[3].parse().expect("decimal Carbon modifier mask");
    match args[1].as_str() {
        "system" => println!("{:?}", carbon::system_shortcut_enabled(key, modifiers)),
        "probe" => match carbon::Registration::new(key, modifiers) {
            Ok(registration) => match registration.release() {
                Ok(()) => println!("AVAILABLE"),
                Err(status) => panic!("release failed: {status}"),
            },
            Err(carbon::HOT_KEY_EXISTS) => println!("BLOCKED"),
            Err(status) => panic!("registration failed: {status}"),
        },
        "hold" => {
            let options = args[4].parse().expect("registration options");
            let registration = carbon::Registration::with_options(key, modifiers, options)
                .unwrap_or_else(|status| panic!("owner registration failed: {status}"));
            println!("READY");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            registration
                .release()
                .expect("release fixture registration");
        }
        _ => panic!("expected system, probe or hold"),
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This fixture requires macOS.");
    std::process::exit(1);
}
