#![cfg(windows)]
//! Separate process from spectrum unit tests, so the singleton analyzer cannot race.
use rust_low_latency_voice_changer::native_spectrum;
use windows::{
    core::w,
    Win32::{
        Foundation::{LPARAM, RECT, WPARAM},
        Graphics::Gdi::{
            CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetPixel,
            ReleaseDC, SelectObject, HGDIOBJ,
        },
        UI::{
            Controls::DRAWITEMSTRUCT,
            WindowsAndMessaging::{FindWindowW, GetDlgItem, SendMessageW, WM_COMMAND, WM_DRAWITEM},
        },
    },
};

unsafe fn assert_button_labels_visible() {
    let window = FindWindowW(w!("RustLowLatencyVoiceChangerSpectrum"), None).unwrap();
    let screen = GetDC(None);
    assert!(!screen.is_invalid());
    let dc = CreateCompatibleDC(Some(screen));
    let bitmap = CreateCompatibleBitmap(screen, 120, 32);
    let _ = ReleaseDC(None, screen);
    assert!(!dc.is_invalid());
    assert!(!bitmap.is_invalid());
    let previous = SelectObject(dc, HGDIOBJ(bitmap.0));
    // Exercise both mode selections and Freeze on/off with the actual owner-draw handler.
    for selected in [301, 302, 303, 303] {
        SendMessageW(window, WM_COMMAND, Some(WPARAM(selected)), None);
        for (id, width) in [(301, 110), (302, 120), (303, 100)] {
            let item = DRAWITEMSTRUCT {
                hwndItem: GetDlgItem(Some(window), id).unwrap(),
                hDC: dc,
                rcItem: RECT {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: 32,
                },
                ..Default::default()
            };
            SendMessageW(
                window,
                WM_DRAWITEM,
                Some(WPARAM(id as usize)),
                Some(LPARAM(&item as *const _ as isize)),
            );
            let mut ink = 0;
            for y in 6..26 {
                for x in 8..width - 8 {
                    let color = GetPixel(dc, x, y).0;
                    // Exact text colors for selected and unselected buttons (COLORREF).
                    if color == 0x0016140a || color == 0x00ece8e6 {
                        ink += 1;
                    }
                }
            }
            assert!(
                ink > 20,
                "button {id} has no visible label (selected={selected})"
            );
        }
    }
    SelectObject(dc, previous);
    let _ = DeleteObject(HGDIOBJ(bitmap.0));
    let _ = DeleteDC(dc);
}

#[test]
fn native_window_can_focus_close_and_reopen_without_leaking_controller() {
    // SAFETY: this test owns all native calls on one thread for its whole lifetime.
    unsafe {
        assert!(!native_spectrum::is_open());
        native_spectrum::open().unwrap();
        assert!(native_spectrum::is_open());
        assert_button_labels_visible();
        native_spectrum::open().unwrap();
        native_spectrum::shutdown();
        assert!(!native_spectrum::is_open());
        native_spectrum::open().unwrap();
        native_spectrum::shutdown();
        native_spectrum::shutdown();
        assert!(!native_spectrum::is_open());
    }
}
