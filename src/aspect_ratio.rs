use raw_window_handle::HasWindowHandle;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetWindowRect, WM_NCDESTROY, WM_SIZING, WMSZ_BOTTOM, WMSZ_BOTTOMLEFT,
    WMSZ_BOTTOMRIGHT, WMSZ_LEFT, WMSZ_RIGHT, WMSZ_TOP, WMSZ_TOPLEFT, WMSZ_TOPRIGHT,
};

use crate::hwnd;

const SUBCLASS_ID: usize = 0xA5A5;

pub fn lock_aspect_ratio(window: impl HasWindowHandle, aspect_ratio: f64) {
    assert!(
        aspect_ratio.is_finite() && aspect_ratio > 0.0,
        "aspect_ratio must be positive and finite, got {aspect_ratio}"
    );

    let ratio_bits = aspect_ratio.to_bits() as usize;
    // Safety: `hwnd` is a live window; `subclass_proc` has the SUBCLASSPROC ABI.
    let _ =
        unsafe { SetWindowSubclass(hwnd(window), Some(subclass_proc), SUBCLASS_ID, ratio_bits) };
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    ref_data: usize,
) -> LRESULT {
    match msg {
        WM_SIZING => {
            let ratio = f64::from_bits(ref_data as u64);
            // Safety: WM_SIZING's lParam points to the proposed window rect,
            // which the handler is expected to adjust in place.
            let rect = unsafe { &mut *(lparam.0 as *mut RECT) };
            constrain_to_ratio(hwnd, rect, wparam.0 as u32, ratio);
            LRESULT(1)
        }
        WM_NCDESTROY => {
            // Safety: we registered the subclass, so we own it's destructor
            let _ = unsafe { RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID) };
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        _ => unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) },
    }
}

fn constrain_to_ratio(hwnd: HWND, rect: &mut RECT, edge: u32, ratio: f64) {
    let Some((horizontal, vertical)) = drag_anchors(edge) else {
        return;
    };
    let Some((frame_w, frame_h)) = frame_size(hwnd) else {
        return;
    };

    // Proposed client size = proposed outer size minus the frame.
    let client_w = (rect.right - rect.left - frame_w).max(0) as f64;
    let client_h = (rect.bottom - rect.top - frame_h).max(0) as f64;

    // Side edges and corners are width-driven; top/bottom edges (the ones
    // that center the horizontal axis) are height-driven.
    let (w, h) = if matches!(horizontal, Anchor::Center) {
        (client_h * ratio, client_h)
    } else {
        (client_w, client_w / ratio)
    };

    // Convert back to outer dimensions and re-anchor each axis.
    let outer_w = w.round() as i32 + frame_w;
    let outer_h = h.round() as i32 + frame_h;
    (rect.left, rect.right) = anchored_span(rect.left, rect.right, outer_w, horizontal);
    (rect.top, rect.bottom) = anchored_span(rect.top, rect.bottom, outer_h, vertical);
}

/// The point of an axis that stays fixed while the window is resized.
enum Anchor {
    /// Keep the left/top coordinate; the right/bottom edge moves.
    Start,
    /// Keep the right/bottom coordinate; the left/top edge moves.
    End,
    /// Keep the midpoint; both edges move symmetrically.
    Center,
}

fn drag_anchors(edge: u32) -> Option<(Anchor, Anchor)> {
    use Anchor::*;
    Some(match edge {
        WMSZ_LEFT => (End, Center),
        WMSZ_RIGHT => (Start, Center),
        WMSZ_TOP => (Center, End),
        WMSZ_BOTTOM => (Center, Start),
        WMSZ_TOPLEFT => (End, End),
        WMSZ_TOPRIGHT => (Start, End),
        WMSZ_BOTTOMLEFT => (End, Start),
        WMSZ_BOTTOMRIGHT => (Start, Start),
        _ => return None,
    })
}

fn anchored_span(start: i32, end: i32, len: i32, anchor: Anchor) -> (i32, i32) {
    match anchor {
        Anchor::Start => (start, start + len),
        Anchor::End => (end - len, end),
        Anchor::Center => {
            let start = (start + end) / 2 - len / 2;
            (start, start + len)
        }
    }
}

fn frame_size(hwnd: HWND) -> Option<(i32, i32)> {
    let mut outer = RECT::default();
    let mut client = RECT::default();
    // Safety: both pointers refer to locally owned, exclusively borrowed RECTs.
    unsafe {
        GetWindowRect(hwnd, &mut outer).ok()?;
        GetClientRect(hwnd, &mut client).ok()?;
    }
    Some((
        (outer.right - outer.left) - (client.right - client.left),
        (outer.bottom - outer.top) - (client.bottom - client.top),
    ))
}
