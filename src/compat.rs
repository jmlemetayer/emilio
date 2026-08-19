//! The Win32 calls the rest of the application would rather not make itself.
//!
//! Every function here is safe to call, and each one discharges inside itself whatever Windows
//! needs in order to make that true, such as dispatching a message only to the queue it was just
//! taken from. What comes back is an ordinary Rust value, so nothing outside this module handles a
//! raw pointer, a handle or a message.
//!
//! That is the point of gathering them: `unsafe_code` is denied for the rest of the crate, so this
//! file is the whole of it, and the only place to look when the operating system does something
//! unexpected.

use std::ptr;

use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PostThreadMessageW, WM_QUIT,
};

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
