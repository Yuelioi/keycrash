use block2::RcBlock;
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags, NSEventType};
use std::cell::RefCell;
use std::collections::HashSet;
use std::ptr::NonNull;

pub struct LocalKeyCapture(Retained<AnyObject>);

impl Drop for LocalKeyCapture {
    fn drop(&mut self) {
        // The token came from addLocalMonitor, and is removed on the UI thread.
        unsafe { NSEvent::removeMonitor(&self.0) };
    }
}

// Local AppKit events only: no event tap, global polling, input injection or
// Accessibility permission. Carbon accepts the same physical keyCode directly.
pub fn install(
    armed: impl Fn() -> bool + 'static,
    on_key: impl Fn(u16, &str) + 'static,
) -> Option<LocalKeyCapture> {
    let consumed = RefCell::new(HashSet::new());
    let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        // Suppress repeats and the corresponding release even after the first
        // key changes the view. Holding Return must not start another check.
        let key = unsafe { event.as_ref() };
        if key.r#type() == NSEventType::KeyUp {
            return if consumed.borrow_mut().remove(&key.keyCode()) {
                std::ptr::null_mut()
            } else {
                event.as_ptr()
            };
        }
        if consumed.borrow().contains(&key.keyCode()) && key.isARepeat() {
            return std::ptr::null_mut();
        }
        // A fresh down also recovers after a release happened in another app.
        consumed.borrow_mut().remove(&key.keyCode());
        if !armed() {
            return event.as_ptr();
        }
        consumed.borrow_mut().insert(key.keyCode());
        // AppKit invokes local event monitors on the main thread.
        let forbidden = NSEventModifierFlags::Control
            | NSEventModifierFlags::Option
            | NSEventModifierFlags::Shift
            | NSEventModifierFlags::Command;
        // Fn can select the F-key layer. Other Fn combinations must not be
        // silently probed as an unmodified physical key.
        let unsupported_fn = key.modifierFlags().contains(NSEventModifierFlags::Function)
            && !crate::mac_keys::is_function_key(key.keyCode());
        if !key.isARepeat() && !key.modifierFlags().intersects(forbidden) && !unsupported_fn {
            let text = key
                .charactersIgnoringModifiers()
                .map(|s| s.to_string())
                .unwrap_or_default();
            on_key(key.keyCode(), &text);
        }
        // Consume while armed, so Return/Space does not activate the next action.
        std::ptr::null_mut()
    });
    unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(
            NSEventMask::KeyDown | NSEventMask::KeyUp,
            &block,
        )
    }
    .map(LocalKeyCapture)
}
