//! The wickra-terminal C ABI — the hub every C-capable language links against.
//!
//! The surface is deliberately tiny and JSON-shaped, exactly like the
//! backtester's `run_json`: construct a `Terminal` from a JSON config, drive it
//! with a command JSON and read back a frame JSON, and free the handle. No
//! terminal type crosses the boundary by value — the handle is opaque and the
//! data is always a NUL-terminated JSON string the callee allocates and the
//! caller frees with [`wickra_terminal_free_string`].
//!
//! Every function null-guards its pointer arguments and returns
//! [`WICKRA_TERMINAL_ERR_NULL`] rather than dereferencing a null pointer.

use core::ffi::{c_char, c_int, CStr};
use std::ffi::CString;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

use terminal_core::Terminal;

/// Success.
pub const WICKRA_TERMINAL_OK: c_int = 0;
/// A required pointer argument was null.
pub const WICKRA_TERMINAL_ERR_NULL: c_int = -1;
/// The operation failed; the error message is written to the out string.
pub const WICKRA_TERMINAL_ERR: c_int = -2;

/// An opaque handle to a terminal instance. Created by [`wickra_terminal_new`]
/// and destroyed by [`wickra_terminal_free`]; never dereferenced by the caller.
pub struct WickraTerminal(Terminal);

/// Read a NUL-terminated C string as `&str`, or `None` on null / bad UTF-8.
///
/// # Safety
/// `ptr` must be null or a valid NUL-terminated C string.
unsafe fn opt_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

/// Construct a terminal from a JSON config string.
///
/// Returns an opaque handle, or null if `config_json` is null, not valid UTF-8,
/// or not a valid config. Free the handle with [`wickra_terminal_free`].
///
/// # Safety
/// `config_json` must be null or a valid NUL-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn wickra_terminal_new(config_json: *const c_char) -> *mut WickraTerminal {
    let Some(json) = (unsafe { opt_str(config_json) }) else {
        return ptr::null_mut();
    };
    match catch_unwind(AssertUnwindSafe(|| Terminal::from_json(json))) {
        Ok(Ok(terminal)) => Box::into_raw(Box::new(WickraTerminal(terminal))),
        _ => ptr::null_mut(),
    }
}

/// Destroy a terminal handle. Null is a no-op.
///
/// # Safety
/// `handle` must be null or a handle previously returned by
/// [`wickra_terminal_new`] and not already freed.
#[no_mangle]
pub unsafe extern "C" fn wickra_terminal_free(handle: *mut WickraTerminal) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}

/// Apply a command JSON and write the resulting frame JSON to `*out_json`.
///
/// On success returns [`WICKRA_TERMINAL_OK`] and `*out_json` points to a
/// NUL-terminated frame JSON. On failure returns [`WICKRA_TERMINAL_ERR`] with
/// the error message in `*out_json`, or [`WICKRA_TERMINAL_ERR_NULL`] if a
/// required pointer is null. The caller frees `*out_json` with
/// [`wickra_terminal_free_string`].
///
/// # Safety
/// `handle` must be a valid handle; `cmd_json` a valid C string; `out_json` a
/// valid pointer to write the result pointer into.
#[no_mangle]
pub unsafe extern "C" fn wickra_terminal_command(
    handle: *mut WickraTerminal,
    cmd_json: *const c_char,
    out_json: *mut *mut c_char,
) -> c_int {
    if out_json.is_null() {
        return WICKRA_TERMINAL_ERR_NULL;
    }
    unsafe { *out_json = ptr::null_mut() };
    if handle.is_null() {
        return WICKRA_TERMINAL_ERR_NULL;
    }
    let Some(cmd) = (unsafe { opt_str(cmd_json) }) else {
        return WICKRA_TERMINAL_ERR_NULL;
    };
    let terminal = unsafe { &mut (*handle).0 };
    let (code, payload) = match catch_unwind(AssertUnwindSafe(|| terminal.command_json(cmd))) {
        Ok(Ok(frame)) => (WICKRA_TERMINAL_OK, frame),
        Ok(Err(err)) => (WICKRA_TERMINAL_ERR, err.to_string()),
        Err(_) => (
            WICKRA_TERMINAL_ERR,
            "panic in wickra_terminal_command".to_string(),
        ),
    };
    match CString::new(payload) {
        Ok(cstr) => {
            unsafe { *out_json = cstr.into_raw() };
            code
        }
        Err(_) => WICKRA_TERMINAL_ERR,
    }
}

/// Free a string previously returned in `*out_json` by
/// [`wickra_terminal_command`]. Null is a no-op.
///
/// # Safety
/// `s` must be null or a pointer returned by this library and not already freed.
#[no_mangle]
pub unsafe extern "C" fn wickra_terminal_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

/// The library version as a static NUL-terminated string (do not free).
#[no_mangle]
pub extern "C" fn wickra_terminal_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0")
        .as_ptr()
        .cast::<c_char>()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r#"{"sources":[{"Synth":{"seed":1}}],"layout":{"panels":[{"kind":"Chart","rect":{"x":0,"y":0,"w":100,"h":100}}]}}"#;

    /// Take ownership of an out-string, returning it as a Rust `String`.
    unsafe fn take(out: *mut c_char) -> String {
        let s = unsafe { CStr::from_ptr(out) }.to_str().unwrap().to_string();
        unsafe { wickra_terminal_free_string(out) };
        s
    }

    #[test]
    fn new_command_free_round_trip() {
        let config = CString::new(CONFIG).unwrap();
        let handle = unsafe { wickra_terminal_new(config.as_ptr()) };
        assert!(!handle.is_null());

        let subscribe =
            CString::new(r#"{"type":"Subscribe","source":0,"symbol":"BTC/USDT"}"#).unwrap();
        let mut out: *mut c_char = ptr::null_mut();
        let code =
            unsafe { wickra_terminal_command(handle, subscribe.as_ptr(), ptr::addr_of_mut!(out)) };
        assert_eq!(code, WICKRA_TERMINAL_OK);
        let frame = unsafe { take(out) };
        assert!(frame.contains("\"panel\":\"chart\""));

        let tick = CString::new(r#"{"type":"Tick"}"#).unwrap();
        let mut out2: *mut c_char = ptr::null_mut();
        let code =
            unsafe { wickra_terminal_command(handle, tick.as_ptr(), ptr::addr_of_mut!(out2)) };
        assert_eq!(code, WICKRA_TERMINAL_OK);
        let _ = unsafe { take(out2) };

        unsafe { wickra_terminal_free(handle) };
    }

    #[test]
    fn bad_command_reports_error_in_out_string() {
        let config = CString::new(CONFIG).unwrap();
        let handle = unsafe { wickra_terminal_new(config.as_ptr()) };
        let bad = CString::new(r#"{"type":"Nope"}"#).unwrap();
        let mut out: *mut c_char = ptr::null_mut();
        let code = unsafe { wickra_terminal_command(handle, bad.as_ptr(), ptr::addr_of_mut!(out)) };
        assert_eq!(code, WICKRA_TERMINAL_ERR);
        assert!(!out.is_null());
        let _ = unsafe { take(out) };
        unsafe { wickra_terminal_free(handle) };
    }

    #[test]
    fn null_config_yields_null_handle() {
        let handle = unsafe { wickra_terminal_new(ptr::null()) };
        assert!(handle.is_null());
    }

    #[test]
    fn null_guards_on_command() {
        // Null out pointer.
        let code =
            unsafe { wickra_terminal_command(ptr::null_mut(), ptr::null(), ptr::null_mut()) };
        assert_eq!(code, WICKRA_TERMINAL_ERR_NULL);
        // Null handle with a valid out pointer.
        let mut out: *mut c_char = ptr::null_mut();
        let code = unsafe {
            wickra_terminal_command(ptr::null_mut(), ptr::null(), ptr::addr_of_mut!(out))
        };
        assert_eq!(code, WICKRA_TERMINAL_ERR_NULL);
        assert!(out.is_null());
    }

    #[test]
    fn free_null_is_a_noop() {
        unsafe { wickra_terminal_free(ptr::null_mut()) };
        unsafe { wickra_terminal_free_string(ptr::null_mut()) };
    }

    #[test]
    fn version_is_nul_terminated() {
        let p = wickra_terminal_version();
        let v = unsafe { CStr::from_ptr(p) }.to_str().unwrap();
        assert_eq!(v, env!("CARGO_PKG_VERSION"));
    }
}
