use std::io::{self, Write};
use std::thread;
use std::time::Duration;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, RegisterHotKey, UnregisterHotKey,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetMessageW, MSG, PostThreadMessageW, WM_HOTKEY,
};

const FIXTURE_ID: i32 = 0x4B46;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let (modifiers, virtual_key, post_delay_ms) = parse_arguments()?;
    let registered =
        unsafe { RegisterHotKey(core::ptr::null_mut(), FIXTURE_ID, modifiers, virtual_key) };
    if registered == 0 {
        return Err(format!("RegisterHotKey failed: {}", unsafe {
            GetLastError()
        }));
    }

    let thread_id = unsafe { GetCurrentThreadId() };
    println!("READY\t{}\t{}", unsafe { GetCurrentProcessId() }, thread_id);
    io::stdout()
        .flush()
        .map_err(|error| format!("flush failed: {error}"))?;

    let message_modifiers = modifiers & (MOD_ALT | MOD_CONTROL | MOD_SHIFT | MOD_WIN);
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(post_delay_ms));
        let hotkey = message_modifiers | (virtual_key << 16);
        unsafe {
            PostThreadMessageW(thread_id, WM_HOTKEY, FIXTURE_ID as usize, hotkey as isize);
        }
    });

    let mut message = MSG::default();
    let result = unsafe { GetMessageW(&mut message, core::ptr::null_mut(), 0, 0) };
    unsafe {
        UnregisterHotKey(core::ptr::null_mut(), FIXTURE_ID);
    }

    if result <= 0 {
        return Err(format!("GetMessageW failed: {}", unsafe { GetLastError() }));
    }

    println!("MESSAGE\t{}\t{}", message.message, message.lParam);
    Ok(())
}

fn parse_arguments() -> Result<(u32, u32, u64), String> {
    let mut arguments = std::env::args().skip(1);
    let modifiers = arguments
        .next()
        .ok_or("missing modifiers")?
        .parse()
        .map_err(|_| "invalid modifiers")?;
    let virtual_key = arguments
        .next()
        .ok_or("missing virtual key")?
        .parse()
        .map_err(|_| "invalid virtual key")?;
    let post_delay_ms = arguments
        .next()
        .map(|value| value.parse().map_err(|_| "invalid post delay"))
        .transpose()?
        .unwrap_or(500);
    Ok((modifiers, virtual_key, post_delay_ms))
}
