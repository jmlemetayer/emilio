//! The Win32 calls the rest of the application would rather not make itself.
//!
//! Every function here is safe to call, and each one discharges inside itself whatever Windows
//! needs in order to make that true: a pointer freed exactly once, a message dispatched only to
//! the queue it was just taken from, a buffer read no further than its terminator. What comes back
//! is an ordinary Rust value, so nothing outside this module handles a raw pointer or a handle.
//!
//! That is the point of gathering them: `unsafe_code` is denied for the rest of the crate, so this
//! file is the whole of it, and the only place to look when the operating system does something
//! unexpected.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr;

use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PostThreadMessageW, WM_QUIT,
};
use windows_sys::core::GUID;

/// Which thread is calling.
///
/// The answer is only useful to hand to [`post_quit`], which is the one thing that needs a thread
/// named from outside the thread itself.
pub fn current_thread() -> u32 {
    // SAFETY: takes nothing and reads a value the operating system keeps for every thread.
    unsafe { GetCurrentThreadId() }
}

/// Asks a thread's message loop to finish, waking it if it is waiting for a message.
///
/// Nothing happens if that thread has already gone, or never had a message loop.
pub fn post_quit(thread: u32) {
    // SAFETY: the operating system checks the thread id and fails the call when it names nothing,
    // so an id that has gone stale posts to no one rather than to somebody else.
    unsafe { PostThreadMessageW(thread, WM_QUIT, 0, 0) };
}

/// Waits for the calling thread's next message and gives it to whatever registered for it.
///
/// Returns `false` when the loop should stop, which is a quit message or an error reading one.
/// Call it from the thread whose messages are wanted: a message queue belongs to one thread and
/// cannot be pumped from another.
pub fn dispatch_next_message() -> bool {
    // SAFETY: MSG is plain data whose zeroed form is valid, GetMessageW fills it before anything
    // reads it, and the message dispatched is the one it just filled. Both calls happen on the
    // calling thread, which is the queue GetMessageW read from.
    unsafe {
        let mut message: MSG = std::mem::zeroed();

        if GetMessageW(&mut message, ptr::null_mut(), 0, 0) <= 0 {
            return false;
        }

        DispatchMessageW(&message);
    }

    true
}

/// Where Windows currently keeps one of the folders it names, if it will say.
///
/// The answer is asked for rather than assembled out of the home directory because these folders
/// move: the player can put Saved Games on another drive, and Windows is the only thing that knows
/// they did.
pub fn known_folder(folder: GUID) -> Option<PathBuf> {
    let mut wide = ptr::null_mut();

    // SAFETY: the call writes one pointer and nothing else. It is freed below whether or not this
    // succeeded, and a failed call leaves it null, which CoTaskMemFree accepts.
    let outcome = unsafe { SHGetKnownFolderPath(&raw const folder, 0, ptr::null_mut(), &mut wide) };

    let path = (outcome >= 0 && !wide.is_null()).then(|| {
        // SAFETY: on success the shell leaves a null-terminated wide string at this pointer, valid
        // until it is freed below.
        PathBuf::from(unsafe { owned(wide) })
    });

    // SAFETY: frees what the shell allocated, exactly once, and accepts the null left by failure.
    unsafe { CoTaskMemFree(wide.cast()) };

    path
}

/// Copies a null-terminated wide string out of memory belonging to somebody else.
///
/// # Safety
///
/// `wide` must point at a null-terminated sequence of `u16` that stays valid for this call.
unsafe fn owned(wide: *const u16) -> OsString {
    let mut length = 0;

    // SAFETY: the caller promises a terminator, which stops this inside the allocation.
    while unsafe { *wide.add(length) } != 0 {
        length += 1;
    }

    // SAFETY: length is the distance to the terminator, so the slice stays inside the string.
    OsString::from_wide(unsafe { std::slice::from_raw_parts(wide, length) })
}
