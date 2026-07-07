//! Rust side of the KPMIO trampoline shim.
//!
//! libkpm's IO callbacks are variadic C functions which stable Rust cannot
//! define, so `shim/kpmio_shim.c` provides variadic trampolines that format
//! the message and forward it to the non-variadic handlers registered here.
//! KPMIO has no user-data pointer, so the handler is process-global.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::sync::Mutex;

use crate::types::KPMIO;

/// Events emitted by libkpm during operations.
#[derive(Debug, Clone)]
pub enum IoEvent {
    /// A log line with an `enum KPMVerbosity` level.
    Log { verbosity: c_int, message: String },
    /// A raw streamed character (used for subprocess output passthrough).
    Stream(char),
    /// A progress update, 0-100, with a status message.
    Progress { percent: u32, message: String },
}

/// Handler for KPMIO events. `on_input` returns the user's yes/no answer to a
/// confirmation prompt and may block until the user responds.
pub trait IoHandler: Send {
    fn on_event(&self, event: IoEvent);
    fn on_input(&self, prompt: &str) -> bool;
}

static HANDLER: Mutex<Option<Box<dyn IoHandler>>> = Mutex::new(None);

extern "C" {
    fn kinstaller_kpmio_set_handlers(
        log: extern "C" fn(c_int, *const c_char),
        stream: extern "C" fn(c_char),
        progress: extern "C" fn(c_uint, *const c_char),
        input: extern "C" fn(*const c_char) -> bool,
    );
    fn kinstaller_kpmio_log_fn() -> *mut c_void;
    fn kinstaller_kpmio_stream_fn() -> *mut c_void;
    fn kinstaller_kpmio_progress_fn() -> *mut c_void;
    fn kinstaller_kpmio_input_fn() -> *mut c_void;
}

fn cstr_lossy(s: *const c_char) -> String {
    if s.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned()
    }
}

extern "C" fn handle_log(verbosity: c_int, message: *const c_char) {
    if let Some(h) = HANDLER.lock().unwrap().as_ref() {
        h.on_event(IoEvent::Log {
            verbosity,
            message: cstr_lossy(message),
        });
    }
}

extern "C" fn handle_stream(c: c_char) {
    if let Some(h) = HANDLER.lock().unwrap().as_ref() {
        h.on_event(IoEvent::Stream(c as u8 as char));
    }
}

extern "C" fn handle_progress(percent: c_uint, message: *const c_char) {
    if let Some(h) = HANDLER.lock().unwrap().as_ref() {
        h.on_event(IoEvent::Progress {
            percent,
            message: cstr_lossy(message),
        });
    }
}

extern "C" fn handle_input(prompt: *const c_char) -> bool {
    if let Some(h) = HANDLER.lock().unwrap().as_ref() {
        h.on_input(&cstr_lossy(prompt))
    } else {
        false
    }
}

/// Register the global IO handler and wire the C trampolines to it.
pub fn set_io_handler(handler: Box<dyn IoHandler>) {
    *HANDLER.lock().unwrap() = Some(handler);
    unsafe {
        kinstaller_kpmio_set_handlers(handle_log, handle_stream, handle_progress, handle_input);
    }
}

/// Remove the global IO handler.
pub fn clear_io_handler() {
    *HANDLER.lock().unwrap() = None;
}

/// Build a `struct KPMIO` whose members point at the C trampolines.
pub fn kpm_io() -> KPMIO {
    unsafe {
        KPMIO {
            log: kinstaller_kpmio_log_fn(),
            stream: kinstaller_kpmio_stream_fn(),
            logProgress: kinstaller_kpmio_progress_fn(),
            getInput: kinstaller_kpmio_input_fn(),
        }
    }
}
