use std::ops::BitAnd;

use bon::Builder;
use windows::{
    Win32::{
        Foundation::{E_UNEXPECTED, GetLastError, HWND},
        UI::WindowsAndMessaging::{
            IDABORT, IDCANCEL, IDCONTINUE, IDIGNORE, IDNO, IDOK as IDOKAY, IDRETRY, IDTRYAGAIN,
            IDYES, MB_ABORTRETRYIGNORE, MB_APPLMODAL,
            MB_CANCELTRYCONTINUE as MB_CANCELTRYAGAINCONTINUE, MB_DEFBUTTON1, MB_DEFBUTTON2,
            MB_DEFBUTTON3, MB_DEFBUTTON4, MB_HELP, MB_ICONASTERISK, MB_ICONERROR,
            MB_ICONEXCLAMATION, MB_ICONHAND, MB_ICONINFORMATION, MB_ICONQUESTION, MB_ICONSTOP,
            MB_ICONWARNING, MB_OK as MB_OKAY, MB_OKCANCEL as MB_OKAYCANCEL, MB_RETRYCANCEL,
            MB_RIGHT, MB_RTLREADING, MB_SERVICE_NOTIFICATION, MB_SETFOREGROUND, MB_SYSTEMMODAL,
            MB_TASKMODAL, MB_TOPMOST, MB_YESNO, MB_YESNOCANCEL, MESSAGEBOX_RESULT,
            MESSAGEBOX_STYLE, MessageBoxW,
        },
    },
    core::{HSTRING, PCWSTR, Result},
};

#[cfg(feature = "serde")]
use serde::Deserialize;

fn pass_text(text: &str) -> PCWSTR {
    PCWSTR::from_raw(HSTRING::from(text).as_ptr())
}

struct Flags(MESSAGEBOX_STYLE);

impl Flags {
    fn new() -> Self {
        Self(MESSAGEBOX_STYLE::default())
    }
    #[inline]
    fn apply(self, flag: MESSAGEBOX_STYLE) -> Self {
        Self(self.0.bitand(flag))
    }

    #[inline]
    fn maybe_apply(self, flag: Option<MESSAGEBOX_STYLE>) -> Self {
        if let Some(flag) = flag {
            self.apply(flag)
        } else {
            self
        }
    }

    #[inline]
    fn define(self, arg: bool, c: MESSAGEBOX_STYLE) -> Self {
        self.maybe_apply(arg.then_some(c))
    }

    fn finish(self) -> MESSAGEBOX_STYLE {
        self.0
    }
}

#[cfg_attr(feature = "serde", derive(Deserialize))]
pub enum Icon {
    /// An exclamation-point icon appears in the message box.
    Exclamation,
    /// An exclamation-point icon appears in the message box.
    Warning,
    /// An icon consisting of a lowercase letter i in a circle appears in the message box.
    Information,
    /// An icon consisting of a lowercase letter i in a circle appears in the message box.
    Asterisk,
    /// A question-mark icon appears in the message box.
    /// The question-mark message icon is no longer recommended
    /// because it does not clearly represent a specific type of message
    /// and because the phrasing of a message as a question could apply to any message type.
    /// In addition, users can confuse the message symbol question mark with Help information.
    /// Therefore, do not use this question mark message symbol in your message boxes.
    /// The system continues to support its inclusion only for backward compatibility.
    Question,
    /// A stop-sign icon appears in the message box.
    Stop,
    /// A stop-sign icon appears in the message box.
    Error,
    /// A stop-sign icon appears in the message box.
    Hand,
}

impl Icon {
    fn flags(self) -> MESSAGEBOX_STYLE {
        match self {
            Self::Exclamation => MB_ICONEXCLAMATION,
            Self::Warning => MB_ICONWARNING,
            Self::Information => MB_ICONINFORMATION,
            Self::Asterisk => MB_ICONASTERISK,
            Self::Question => MB_ICONQUESTION,
            Self::Stop => MB_ICONSTOP,
            Self::Error => MB_ICONERROR,
            Self::Hand => MB_ICONHAND,
        }
    }
}

#[cfg_attr(feature = "serde", derive(Deserialize))]
pub enum DefaultButton {
    First,
    Second,
    Third,
    Fourth,
}

impl DefaultButton {
    fn flags(self) -> MESSAGEBOX_STYLE {
        match self {
            Self::First => MB_DEFBUTTON1,
            Self::Second => MB_DEFBUTTON2,
            Self::Third => MB_DEFBUTTON3,
            Self::Fourth => MB_DEFBUTTON4,
        }
    }
}

pub trait MessageOptions: Sized {
    fn flags() -> MESSAGEBOX_STYLE;
    fn parse(value: MESSAGEBOX_RESULT) -> Option<Self>;
}

macro_rules! mbox {
    (@define $name:ident = $flag:ident { $($repr:ident => $variant:ident),* $(,)? }) => {
        pub enum $name {
            $($variant),*
        }

        impl MessageOptions for $name {
            fn flags() -> MESSAGEBOX_STYLE {
                $flag
            }

            fn parse(value: MESSAGEBOX_RESULT) -> Option<Self> {
                Some(match value {
                    $($repr => Self::$variant,)*
                    _ => return None,
                })
            }
        }
    };

    ($($variant:ident)*) => {
        pastey::paste! { mbox! { @define
            [< $($variant)* >] = [<MB_ $($variant:upper)*>] {
                $([<ID $variant:upper>] => $variant),*
            }
        } }
    }
}

mbox!(Okay);
mbox!(Okay Cancel);
mbox!(Retry Cancel);
mbox!(Abort Retry Ignore);
mbox!(Cancel TryAgain Continue);
mbox!(Yes No);
mbox!(Yes No Cancel);

#[derive(Builder)]
#[builder(finish_fn = save)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct Message<'a> {
    title: Option<&'a str>,
    text: &'a str,
    icon: Option<Icon>,

    #[builder(setters(vis = ""))]
    #[cfg_attr(feature = "serde", serde(skip))]
    hwnd: Option<HWND>,

    #[builder(setters(vis = ""), default)]
    system_modal: bool,

    default_button: Option<DefaultButton>,

    #[builder(default)]
    topmost: bool,
    #[builder(default)]
    become_foreground_window: bool,
    #[builder(default)]
    right_justify_text: bool,
    #[builder(default)]
    rtl_text: bool,
    #[builder(default)]
    is_service_notification: bool,
    #[builder(default)]
    has_help: bool,
}

impl<'a> Message<'a> {
    pub fn display<T: MessageOptions>(self) -> Result<T> {
        let modal = match self.hwnd.is_some() {
            true if self.system_modal => MB_SYSTEMMODAL,
            true => MB_APPLMODAL,
            false => MB_TASKMODAL,
        };

        let flags = Flags::new()
            .apply(T::flags())
            .apply(modal)
            .maybe_apply(self.icon.map(Icon::flags))
            .maybe_apply(self.default_button.map(DefaultButton::flags))
            .define(self.topmost, MB_TOPMOST)
            .define(self.become_foreground_window, MB_SETFOREGROUND)
            .define(self.right_justify_text, MB_RIGHT)
            .define(self.rtl_text, MB_RTLREADING)
            .define(self.is_service_notification, MB_SERVICE_NOTIFICATION)
            .define(self.has_help, MB_HELP)
            .finish();

        let ret = unsafe {
            MessageBoxW(
                self.hwnd,
                pass_text(self.text),
                self.title.map_or_default(pass_text),
                flags,
            )
        };

        match ret {
            MESSAGEBOX_RESULT(0) => Err(unsafe { GetLastError().into() }),
            x => T::parse(x).ok_or(E_UNEXPECTED.into()),
        }
    }
}

mod message_builder_ext {
    use raw_window_handle::HasWindowHandle;
    use windows::core::Result;

    use super::{MessageBuilder, MessageOptions, message_builder::*};

    impl<'a, S: State> MessageBuilder<'a, S> {
        pub fn window(self, window: impl HasWindowHandle) -> MessageBuilder<'a, SetHwnd<S>>
        where
            S::Hwnd: IsUnset,
        {
            self.hwnd(crate::hwnd(window))
        }

        pub fn block_other_windows(self) -> MessageBuilder<'a, SetSystemModal<S>>
        where
            S::SystemModal: IsUnset,
        {
            self.system_modal(true)
        }

        pub fn display<T: MessageOptions>(self) -> Result<T>
        where
            S: IsComplete,
        {
            let message = self.save();
            message.display()
        }
    }
}
