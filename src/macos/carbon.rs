//! Minimal bindings to the public HIToolbox hotkey APIs (CarbonEvents.h).
//! All calls are made on the main thread. Registration is never kept across
//! an event-loop turn, except by the explicitly invoked test fixture.
use std::ffi::{c_char, c_void};
use std::ptr;

pub const HOT_KEY_EXISTS: i32 = -9878;
const PARAM_ERR: i32 = -50;
type CFRef = *const c_void;

#[repr(C)]
struct HotKeyId {
    signature: u32,
    id: u32,
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn GetApplicationEventTarget() -> *mut c_void;
    fn RegisterEventHotKey(
        key: u32,
        modifiers: u32,
        id: HotKeyId,
        target: *mut c_void,
        options: u32,
        result: *mut *mut c_void,
    ) -> i32;
    fn UnregisterEventHotKey(key: *mut c_void) -> i32;
    fn CopySymbolicHotKeys(result: *mut CFRef) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: CFRef);
    fn CFGetTypeID(value: CFRef) -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFDictionaryGetTypeID() -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFArrayGetCount(array: CFRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CFRef, index: isize) -> CFRef;
    fn CFDictionaryGetValue(dictionary: CFRef, key: CFRef) -> CFRef;
    fn CFNumberGetValue(number: CFRef, number_type: isize, value: *mut i64) -> u8;
    fn CFBooleanGetValue(value: CFRef) -> u8;
    fn CFStringCreateWithCString(allocator: CFRef, text: *const c_char, encoding: u32) -> CFRef;
}

struct OwnedCf(CFRef);
impl Drop for OwnedCf {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

pub struct Registration(*mut c_void);
impl Registration {
    pub fn new(key: u16, modifiers: u32) -> Result<Self, i32> {
        // kEventHotKeyExclusive; default shared registration alone cannot prove
        // absence of another process's registration. See docs/MACOS.md.
        Self::with_options(key, modifiers, 1)
    }

    pub fn with_options(key: u16, modifiers: u32, options: u32) -> Result<Self, i32> {
        let mut handle = ptr::null_mut();
        let status = unsafe {
            RegisterEventHotKey(
                u32::from(key),
                modifiers,
                HotKeyId {
                    signature: u32::from_be_bytes(*b"KCrh"),
                    id: 1,
                },
                GetApplicationEventTarget(),
                options,
                &mut handle,
            )
        };
        if status != 0 {
            return Err(status);
        }
        if handle.is_null() {
            return Err(PARAM_ERR);
        }
        Ok(Self(handle))
    }

    pub fn release(mut self) -> Result<(), i32> {
        let status = unsafe { UnregisterEventHotKey(self.0) };
        if status == 0 {
            self.0 = ptr::null_mut();
            Ok(())
        } else {
            // Drop retries release; the caller still reports the failure.
            Err(status)
        }
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { UnregisterEventHotKey(self.0) };
        }
    }
}

pub fn system_shortcut_enabled(key: u16, modifiers: u32) -> Result<bool, i32> {
    unsafe {
        let mut array = ptr::null();
        let status = CopySymbolicHotKeys(&mut array);
        if status != 0 {
            return Err(status);
        }
        if array.is_null() {
            return Err(PARAM_ERR);
        }
        let array = OwnedCf(array);
        if CFGetTypeID(array.0) != CFArrayGetTypeID() {
            return Err(PARAM_ERR);
        }
        let keys = [
            c"kHISymbolicHotKeyCode",
            c"kHISymbolicHotKeyModifiers",
            c"kHISymbolicHotKeyEnabled",
        ];
        let keys = keys.map(|text| {
            OwnedCf(CFStringCreateWithCString(
                ptr::null(),
                text.as_ptr(),
                0x08000100,
            ))
        });
        if keys.iter().any(|key| key.0.is_null()) {
            return Err(PARAM_ERR);
        }
        for index in 0..CFArrayGetCount(array.0) {
            let entry = CFArrayGetValueAtIndex(array.0, index);
            if entry.is_null() || CFGetTypeID(entry) != CFDictionaryGetTypeID() {
                return Err(PARAM_ERR);
            }
            let code = CFDictionaryGetValue(entry, keys[0].0);
            let flags = CFDictionaryGetValue(entry, keys[1].0);
            let enabled = CFDictionaryGetValue(entry, keys[2].0);
            if code.is_null()
                || flags.is_null()
                || enabled.is_null()
                || CFGetTypeID(code) != CFNumberGetTypeID()
                || CFGetTypeID(flags) != CFNumberGetTypeID()
                || CFGetTypeID(enabled) != CFBooleanGetTypeID()
            {
                return Err(PARAM_ERR);
            }
            let (mut code_value, mut flag_value) = (0i64, 0i64);
            // kCFNumberSInt64Type = 4. Keep the full Carbon mask: Fn and other
            // unsupported flags must not be silently matched as a plain key.
            if CFNumberGetValue(code, 4, &mut code_value) == 0
                || CFNumberGetValue(flags, 4, &mut flag_value) == 0
            {
                return Err(PARAM_ERR);
            }
            if CFBooleanGetValue(enabled) != 0
                && code_value == i64::from(key)
                && flag_value == i64::from(modifiers)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
