//! Windows video surface. mpv draws into its own child window (`--wid`). The child sits
//! below WebView2 in the Lokii window, and the transparent page shows it.

use std::sync::atomic::{AtomicIsize, Ordering};

use tauri::{WebviewWindow, WindowEvent};
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::CreateSolidBrush;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, RegisterClassW, SetWindowPos, HWND_BOTTOM, SWP_NOACTIVATE,
    WINDOW_EX_STYLE, WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS, WS_DISABLED, WS_VISIBLE,
};

/// The child window mpv draws into, as mpv's `wid` value. 0 until it exists.
static VIDEO_WINDOW: AtomicIsize = AtomicIsize::new(0);

/// Creates the video child window on the main thread and keeps it at the window's size.
pub fn attach_video_window(window: &WebviewWindow) -> Result<(), String> {
    let parent = window.hwnd().map_err(|e| format!("no window handle: {e}"))?;
    let child = unsafe { create_child(parent) }.map_err(|e| format!("cannot create the video window: {e}"))?;
    let handle = child.0 as isize;
    VIDEO_WINDOW.store(handle, Ordering::SeqCst);
    window.on_window_event(move |event| {
        if let WindowEvent::Resized(size) = event {
            place(HWND(handle as _), size.width as i32, size.height as i32);
        }
    });
    Ok(())
}

/// The `wid` for mpv.
pub fn video_window() -> i64 {
    VIDEO_WINDOW.load(Ordering::SeqCst) as i64
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// A black, disabled child window: WebView2 above it gets all mouse and keyboard input.
unsafe fn create_child(parent: HWND) -> windows::core::Result<HWND> {
    let instance = unsafe { GetModuleHandleW(None)? };
    let class_name = w!("LokiiVideo");
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance.into(),
        hbrBackground: unsafe { CreateSolidBrush(COLORREF(0)) },
        lpszClassName: class_name,
        ..Default::default()
    };
    unsafe { RegisterClassW(&class) };
    let mut rect = RECT::default();
    unsafe { GetClientRect(parent, &mut rect)? };
    let style = WS_CHILD | WS_VISIBLE | WS_DISABLED | WS_CLIPSIBLINGS;
    let child = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!(""),
            style,
            0,
            0,
            rect.right,
            rect.bottom,
            Some(parent),
            None,
            Some(instance.into()),
            None,
        )?
    };
    place(child, rect.right, rect.bottom);
    Ok(child)
}

/// Fills the window and stays below WebView2.
fn place(child: HWND, width: i32, height: i32) {
    let _ = unsafe { SetWindowPos(child, Some(HWND_BOTTOM), 0, 0, width, height, SWP_NOACTIVATE) };
}
