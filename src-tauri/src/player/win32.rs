//! Windows video surface. mpv gets the Lokii window itself as `--wid`: it creates its own
//! child window there and keeps it at the window's size. WebView2 draws above that child,
//! and the transparent page shows it.
//!
//! An extra child window of our own between the two stays invisible: the transparent
//! window drops what GDI paints into it, and mpv's picture inside it never reaches the screen.

use std::sync::atomic::{AtomicIsize, Ordering};

use tauri::WebviewWindow;

/// The Lokii window, as mpv's `wid` value. 0 until it is known.
static VIDEO_WINDOW: AtomicIsize = AtomicIsize::new(0);

/// Remembers the Lokii window for mpv.
pub fn attach_video_window(window: &WebviewWindow) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(|e| format!("no window handle: {e}"))?;
    VIDEO_WINDOW.store(hwnd.0 as isize, Ordering::SeqCst);
    Ok(())
}

/// The `wid` for mpv.
pub fn video_window() -> i64 {
    VIDEO_WINDOW.load(Ordering::SeqCst) as i64
}
