// SPDX-License-Identifier: MPL-2.0
//! Logging control for the Prism library.

use prism_sys as sys;
use std::ffi::CString;
use std::sync::Mutex;

pub use prism_types::LogLevel;

/// A boxed log sink: `(level, source, message)`.
type LogBox = Box<dyn Fn(LogLevel, &str, &str) + Send + Sync>;

// The C API stores a single global handler. We keep the Rust closure alive here
// so its heap storage is never freed while the C side holds a pointer to it.
static LOG_HANDLER: Mutex<Option<Box<LogBox>>> = Mutex::new(None);

/// Set the global minimum log level. Returns the previous level.
pub fn set_log_level(level: LogLevel) -> LogLevel {
    // SAFETY: no preconditions.
    let prev = unsafe { sys::prism_set_log_level(level.as_i32() as sys::PrismLogLevel) };
    LogLevel::from_i32(prev as i32).unwrap_or(LogLevel::None)
}

/// Install a global log handler.
///
/// Replaces any previously installed handler. The closure may be called from
/// any thread.
pub fn set_log_handler<F>(handler: F)
where
    F: Fn(LogLevel, &str, &str) + Send + Sync + 'static,
{
    let holder: Box<LogBox> = Box::new(Box::new(handler));
    let userdata = &*holder as *const LogBox as *mut libc::c_void;

    let sys_handler = sys::PrismLogHandler {
        fn_: Some(log_trampoline),
        userdata,
    };
    // Keep the new closure alive *before* installing it, and hold the old one
    // until after the swap so no in-flight call dereferences freed storage.
    let mut guard = LOG_HANDLER.lock().unwrap();
    let previous = guard.replace(holder);
    // SAFETY: no preconditions; returns the prior handler (ignored).
    let _ = unsafe { sys::prism_set_log_handler(sys_handler) };
    drop(guard);
    drop(previous);
}

/// Emit a log record through Prism's logging pipeline.
pub fn log(level: LogLevel, source: &str, message: &str) {
    let (Ok(source), Ok(message)) = (CString::new(source), CString::new(message)) else {
        return;
    };
    // SAFETY: both strings are valid and NUL-terminated for the call.
    unsafe {
        sys::prism_log(
            level.as_i32() as sys::PrismLogLevel,
            source.as_ptr(),
            message.as_ptr(),
        )
    };
}

/// Flush any buffered log records.
pub fn flush() {
    // SAFETY: no preconditions.
    unsafe { sys::prism_log_flush() };
}

/// Shut down the logging subsystem.
pub fn shutdown() {
    // SAFETY: no preconditions.
    unsafe { sys::prism_log_shutdown() };
}

extern "C" fn log_trampoline(
    userdata: *mut libc::c_void,
    level: sys::PrismLogLevel,
    source: *const libc::c_char,
    message: *const libc::c_char,
) {
    if userdata.is_null() {
        return;
    }
    // SAFETY: `userdata` is the stable address of the `LogBox` kept in
    // `LOG_HANDLER`.
    let cb = unsafe { &*(userdata as *const LogBox) };
    let level = LogLevel::from_i32(level).unwrap_or(LogLevel::None);
    let to_str = |p: *const libc::c_char| -> String {
        if p.is_null() {
            String::new()
        } else {
            // SAFETY: valid NUL-terminated string for this call.
            unsafe { core::ffi::CStr::from_ptr(p) }
                .to_string_lossy()
                .into_owned()
        }
    };
    cb(level, &to_str(source), &to_str(message));
}
