#[cfg(feature = "messagebox")]
pub mod messagebox;

#[cfg(feature = "lock-aspect-ratio")]
pub mod aspect_ratio;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::HWND;

#[cfg(not(target_os = "windows"))]
compile_error!("WinWin is only supported on windows");

fn hwnd(window: impl HasWindowHandle) -> HWND {
    match window
        .window_handle()
        .expect("Failed to get window handle")
        .as_raw()
    {
        RawWindowHandle::Win32(h) => HWND(h.hwnd.get() as _),
        _ => unreachable!(), // guarded by compilation check above
    }
}
