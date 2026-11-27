#[cfg(feature = "lock-aspect-ratio")]
pub mod aspect_ratio;
#[cfg(feature = "messagebox")]
pub mod messagebox;
#[cfg(feature = "resources")]
pub mod resources;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::{
    Win32::Foundation::HWND,
    core::{HSTRING, PCWSTR},
};

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

fn pass_text(text: &str) -> PCWSTR {
    PCWSTR::from_raw(HSTRING::from(text).as_ptr())
}

#[cfg(feature = "tracing-layer")]
pub mod tracing_layer {
    use std::io;

    use tracing_core::Metadata;
    use tracing_subscriber::{
        fmt::MakeWriter,
        layer::{Context, Filter},
    };
    use windows::Win32::System::Diagnostics::Debug::{IsDebuggerPresent, OutputDebugStringW};

    use crate::pass_text;

    /// Should be used both
    /// - as a writer - to forward logs as windows debugging events
    /// - as a filter - to disable the tracing stack when no debugger is attached
    pub struct WinDebug;

    impl io::Write for WinDebug {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let text = str::from_utf8(buf).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "DebugOutputWriter only supports utf-8 data",
                )
            })?;

            let arg = pass_text(text);

            unsafe { OutputDebugStringW(arg) };

            Ok(buf.len())
        }

        // TODO: does it make sense to keep an internal buffer?
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for WinDebug {
        type Writer = Self;

        fn make_writer(&'a self) -> Self::Writer {
            Self
        }
    }

    impl<S> Filter<S> for WinDebug {
        fn enabled(&self, _: &Metadata, _: &Context<'_, S>) -> bool {
            unsafe { IsDebuggerPresent() }.as_bool()
        }
    }
}
