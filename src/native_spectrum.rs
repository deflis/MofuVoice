//! Native spectrogram window. All calls and window messages run on the desktop main thread.
use crate::spectrum::{
    MeterLevels, SpectrumController, BYTES_PER_PIXEL, IMAGE_HEIGHT, IMAGE_WIDTH,
};
use std::mem::{size_of, size_of_val};
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::{
    core::w,
    Win32::{
        Foundation::{COLORREF, ERROR_CLASS_ALREADY_EXISTS, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::{
            Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE},
            Gdi::{
                BitBlt, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreatePen,
                CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, FillRect, GetDC,
                InvalidateRect, ReleaseDC, RoundRect, SelectObject, SetBkMode, SetTextColor,
                BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS,
                DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, DT_CENTER, DT_SINGLELINE,
                DT_VCENTER, FW_NORMAL, HBITMAP, HBRUSH, HDC, HFONT, HGDIOBJ, OUT_DEFAULT_PRECIS,
                PS_SOLID, SRCCOPY, TRANSPARENT,
            },
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::{DRAWITEMSTRUCT, ODS_DISABLED, ODS_HOTLIGHT, ODS_SELECTED},
            WindowsAndMessaging::{
                AdjustWindowRect, CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect,
                GetWindowLongPtrW, GetWindowTextW, KillTimer, LoadCursorW, RegisterClassW,
                SendMessageW, SetForegroundWindow, SetTimer, SetWindowLongPtrW, ShowWindow,
                CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, GWLP_USERDATA, HMENU,
                IDC_ARROW, SW_RESTORE, SW_SHOW, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE,
                WM_DESTROY, WM_DRAWITEM, WM_ERASEBKGND, WM_NCCREATE, WM_SETFONT, WM_TIMER,
                WNDCLASSW, WS_CAPTION, WS_CHILD, WS_GROUP, WS_MINIMIZEBOX, WS_OVERLAPPED,
                WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
            },
        },
    },
};

const SPEC_WIDE: usize = 301;
const SPEC_NARROW: usize = 302;
const SPEC_FREEZE: usize = 303;
const BS_OWNERDRAW: WINDOW_STYLE = WINDOW_STYLE(0x000B);
const SS_OWNERDRAW: WINDOW_STYLE = WINDOW_STYLE(0x000D);
const BN_CLICKED: u16 = 0;
const IMAGE_BYTES: usize = IMAGE_WIDTH * IMAGE_HEIGHT * BYTES_PER_PIXEL;

const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | ((g as u32) << 8) | ((b as u32) << 16))
}

mod theme {
    use super::rgb;
    use windows::Win32::Foundation::COLORREF;
    pub const BG: COLORREF = rgb(22, 24, 28);
    pub const BORDER: COLORREF = rgb(55, 60, 70);
    pub const TEXT: COLORREF = rgb(230, 232, 236);
    pub const MUTED: COLORREF = rgb(140, 148, 160);
    pub const ACCENT: COLORREF = rgb(0, 196, 180);
    pub const ACCENT_DIM: COLORREF = rgb(0, 140, 130);
    pub const BUTTON: COLORREF = rgb(42, 46, 54);
    pub const BUTTON_HOT: COLORREF = rgb(52, 58, 68);
}

struct State {
    controller: SpectrumController,
    wide_button: HWND,
    narrow_button: HWND,
    freeze_button: HWND,
    view: HWND,
    meter_view: HWND,
    levels: MeterLevels,
    dib: HBITMAP,
    bits: *mut core::ffi::c_void,
    pixels: Vec<u8>,
    generation: u64,
    frozen: bool,
    wideband: bool,
    started: bool,
    font: HFONT,
    bg_brush: HBRUSH,
    error: u32,
}

static WINDOW: AtomicIsize = AtomicIsize::new(0);

fn window_handle() -> HWND {
    HWND(WINDOW.load(Ordering::SeqCst) as *mut _)
}

fn set_window_handle(window: HWND) {
    WINDOW.store(window.0 as isize, Ordering::SeqCst);
}

/// True while the spectrogram window exists.
pub fn is_open() -> bool {
    !window_handle().0.is_null()
}

unsafe fn enable_dark_title(window: HWND) {
    let dark = windows::core::BOOL(1);
    let _ = DwmSetWindowAttribute(
        window,
        DWMWA_USE_IMMERSIVE_DARK_MODE,
        &dark as *const _ as *const _,
        size_of_val(&dark) as u32,
    );
}

unsafe fn paint_background(window: HWND, dc: HDC, brush: HBRUSH) {
    let mut rect = RECT::default();
    let _ = GetClientRect(window, &mut rect);
    FillRect(dc, &rect, brush);
}

unsafe fn draw_label(dc: HDC, font: HFONT, color: COLORREF, rect: RECT, text: &mut [u16]) {
    let mut rect = rect;
    let previous = SelectObject(dc, HGDIOBJ(font.0));
    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, color);
    DrawTextW(dc, text, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    SelectObject(dc, previous);
}

unsafe fn draw_button(item: &DRAWITEMSTRUCT, state: &State, primary: bool) {
    let disabled = item.itemState.0 & ODS_DISABLED.0 != 0;
    let hot = item.itemState.0 & (ODS_HOTLIGHT.0 | ODS_SELECTED.0) != 0;
    let fill = if primary {
        if disabled {
            theme::ACCENT_DIM
        } else if hot {
            rgb(0, 214, 196)
        } else {
            theme::ACCENT
        }
    } else if disabled {
        rgb(36, 40, 46)
    } else if hot {
        theme::BUTTON_HOT
    } else {
        theme::BUTTON
    };
    let text = if primary {
        if disabled {
            rgb(180, 220, 215)
        } else {
            rgb(10, 20, 22)
        }
    } else if disabled {
        theme::MUTED
    } else {
        theme::TEXT
    };
    let brush = CreateSolidBrush(fill);
    let pen = CreatePen(PS_SOLID, 1, if primary { fill } else { theme::BORDER });
    let old_brush = SelectObject(item.hDC, HGDIOBJ(brush.0));
    let old_pen = SelectObject(item.hDC, HGDIOBJ(pen.0));
    let _ = RoundRect(
        item.hDC,
        item.rcItem.left,
        item.rcItem.top,
        item.rcItem.right,
        item.rcItem.bottom,
        10,
        10,
    );
    SelectObject(item.hDC, old_brush);
    SelectObject(item.hDC, old_pen);
    let _ = DeleteObject(HGDIOBJ(brush.0));
    let _ = DeleteObject(HGDIOBJ(pen.0));
    let mut label = [0u16; 64];
    let length = GetWindowTextW(item.hwndItem, &mut label) as usize;
    draw_label(
        item.hDC,
        state.font,
        text,
        item.rcItem,
        &mut label[..length],
    );
}

unsafe fn fill_meter_rect(dc: HDC, rect: RECT, color: COLORREF) {
    let brush = CreateSolidBrush(color);
    FillRect(dc, &rect, brush);
    let _ = DeleteObject(HGDIOBJ(brush.0));
}

unsafe fn draw_meter(item: &DRAWITEMSTRUCT, state: &State) {
    FillRect(item.hDC, &item.rcItem, state.bg_brush);
    let label = |text: &str, top: i32, bottom: i32, color: COLORREF| {
        let mut text: Vec<u16> = text.encode_utf16().collect();
        draw_label(
            item.hDC,
            state.font,
            color,
            RECT {
                left: 0,
                top,
                right: 144,
                bottom,
            },
            &mut text,
        );
    };
    label("Output 1 / dBFS", 0, 24, theme::TEXT);
    for (left, right, name) in [(4, 44, "RMS"), (44, 82, "Peak")] {
        let mut text: Vec<u16> = name.encode_utf16().collect();
        draw_label(
            item.hDC,
            state.font,
            theme::MUTED,
            RECT {
                left,
                top: 24,
                right,
                bottom: 46,
            },
            &mut text,
        );
    }
    let y = |db: f64| 300 - ((db.clamp(-60.0, 0.0) + 60.0) * 4.2).round() as i32;
    for (left, right, db) in [
        (12, 38, state.levels.rms_db),
        (48, 74, state.levels.peak_db),
    ] {
        fill_meter_rect(
            item.hDC,
            RECT {
                left,
                top: 48,
                right,
                bottom: 300,
            },
            theme::BUTTON,
        );
        for (low, high, color) in [
            (-60.0_f64, -12.0_f64, theme::ACCENT),
            (-12.0, -3.0, rgb(235, 185, 60)),
            (-3.0, 0.0, rgb(235, 75, 75)),
        ] {
            if db > low {
                fill_meter_rect(
                    item.hDC,
                    RECT {
                        left,
                        top: y(db.min(high)),
                        right,
                        bottom: y(low),
                    },
                    color,
                );
            }
        }
    }
    let hold_y = y(state.levels.hold_db);
    fill_meter_rect(
        item.hDC,
        RECT {
            left: 46,
            top: hold_y,
            right: 76,
            bottom: hold_y + 2,
        },
        theme::TEXT,
    );
    for db in [0, -12, -24, -36, -48, -60] {
        let top = y(db as f64) - 10;
        let mut text: Vec<u16> = db.to_string().encode_utf16().collect();
        draw_label(
            item.hDC,
            state.font,
            theme::MUTED,
            RECT {
                left: 82,
                top,
                right: 140,
                bottom: top + 20,
            },
            &mut text,
        );
    }
    label(
        &format!("RMS {:.1}", state.levels.rms_db),
        316,
        340,
        theme::TEXT,
    );
    label(
        &format!("Peak {:.1}", state.levels.peak_db),
        340,
        364,
        theme::TEXT,
    );
    label(
        &format!("Hold {:.1}", state.levels.hold_db),
        364,
        388,
        if state.levels.hold_db >= 0.0 {
            rgb(235, 75, 75)
        } else {
            theme::MUTED
        },
    );
}

unsafe fn destroy_dib(state: &mut State) {
    if !state.dib.is_invalid() {
        let _ = DeleteObject(HGDIOBJ(state.dib.0));
        state.dib = HBITMAP::default();
        state.bits = core::ptr::null_mut();
    }
}

unsafe fn ensure_dib(state: &mut State) -> bool {
    if !state.dib.is_invalid() {
        return true;
    }
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: IMAGE_WIDTH as i32,
            biHeight: -(IMAGE_HEIGHT as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let screen = GetDC(None);
    if screen.is_invalid() {
        return false;
    }
    let mut bits = core::ptr::null_mut();
    let dib = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0);
    let _ = ReleaseDC(None, screen);
    let Ok(dib) = dib else {
        return false;
    };
    if bits.is_null() {
        return false;
    }
    state.dib = dib;
    state.bits = bits;
    core::ptr::write_bytes(bits.cast::<u8>(), 0, IMAGE_BYTES);
    true
}

unsafe fn update_image(state: &mut State) {
    let levels = state.controller.levels();
    if levels != state.levels {
        state.levels = levels;
        let _ = InvalidateRect(Some(state.meter_view), None, false);
    }
    if !ensure_dib(state) {
        state.error = windows::Win32::Foundation::GetLastError().0;
        return;
    }
    if state.pixels.len() != IMAGE_BYTES {
        state.pixels.resize(IMAGE_BYTES, 0);
    }
    if state
        .controller
        .copy_image(&mut state.pixels, &mut state.generation)
    {
        core::ptr::copy_nonoverlapping(state.pixels.as_ptr(), state.bits.cast::<u8>(), IMAGE_BYTES);
        let _ = InvalidateRect(Some(state.view), None, false);
    }
}

unsafe extern "system" fn spectrum_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = lparam.0 as *const CREATESTRUCTW;
        let state = (*create).lpCreateParams as *mut State;
        SetWindowLongPtrW(window, GWLP_USERDATA, state as isize);
        set_window_handle(window);
    }
    let state_ptr = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut State;
    if state_ptr.is_null() {
        return DefWindowProcW(window, message, wparam, lparam);
    }
    let state = &mut *state_ptr;
    match message {
        WM_CREATE => {
            enable_dark_title(window);
            let instance = GetModuleHandleW(None).expect("module handle");
            state.wide_button = CreateWindowExW(
                Default::default(),
                w!("BUTTON"),
                w!("Wideband"),
                WS_CHILD | WS_VISIBLE | BS_OWNERDRAW | WS_GROUP | WS_TABSTOP,
                16,
                16,
                110,
                32,
                Some(window),
                Some(HMENU(SPEC_WIDE as *mut _)),
                Some(instance.into()),
                None,
            )
            .unwrap_or_default();
            state.narrow_button = CreateWindowExW(
                Default::default(),
                w!("BUTTON"),
                w!("Narrowband"),
                WS_CHILD | WS_VISIBLE | BS_OWNERDRAW | WS_TABSTOP,
                136,
                16,
                120,
                32,
                Some(window),
                Some(HMENU(SPEC_NARROW as *mut _)),
                Some(instance.into()),
                None,
            )
            .unwrap_or_default();
            state.freeze_button = CreateWindowExW(
                Default::default(),
                w!("BUTTON"),
                w!("Freeze"),
                WS_CHILD | WS_VISIBLE | BS_OWNERDRAW | WS_TABSTOP,
                266,
                16,
                100,
                32,
                Some(window),
                Some(HMENU(SPEC_FREEZE as *mut _)),
                Some(instance.into()),
                None,
            )
            .unwrap_or_default();
            state.view = CreateWindowExW(
                Default::default(),
                w!("STATIC"),
                w!(""),
                WS_CHILD | WS_VISIBLE | SS_OWNERDRAW,
                16,
                60,
                IMAGE_WIDTH as i32,
                IMAGE_HEIGHT as i32,
                Some(window),
                None,
                Some(instance.into()),
                None,
            )
            .unwrap_or_default();
            state.meter_view = CreateWindowExW(
                Default::default(),
                w!("STATIC"),
                w!(""),
                WS_CHILD | WS_VISIBLE | SS_OWNERDRAW,
                IMAGE_WIDTH as i32 + 32,
                60,
                144,
                IMAGE_HEIGHT as i32,
                Some(window),
                None,
                Some(instance.into()),
                None,
            )
            .unwrap_or_default();
            for item in [state.wide_button, state.narrow_button, state.freeze_button] {
                if item.0.is_null() {
                    state.error = windows::Win32::Foundation::GetLastError().0;
                    return LRESULT(-1);
                }
                SendMessageW(
                    item,
                    WM_SETFONT,
                    Some(WPARAM(state.font.0 as usize)),
                    Some(LPARAM(1)),
                );
            }
            if state.view.0.is_null() || state.meter_view.0.is_null() {
                state.error = windows::Win32::Foundation::GetLastError().0;
                return LRESULT(-1);
            }
            state.started = true;
            state.controller.set_wideband(true);
            if !ensure_dib(state) {
                state.error = windows::Win32::Foundation::GetLastError().0;
                return LRESULT(-1);
            }
            if SetTimer(Some(window), 1, 33, None) == 0 {
                state.error = windows::Win32::Foundation::GetLastError().0;
                return LRESULT(-1);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => {
            paint_background(window, HDC(wparam.0 as *mut _), state.bg_brush);
            LRESULT(1)
        }
        WM_COMMAND => {
            if ((wparam.0 >> 16) as u16) != BN_CLICKED {
                return DefWindowProcW(window, message, wparam, lparam);
            }
            match wparam.0 as u16 as usize {
                SPEC_WIDE => {
                    state.wideband = true;
                    state.controller.set_wideband(true);
                    let _ = InvalidateRect(Some(state.wide_button), None, false);
                    let _ = InvalidateRect(Some(state.narrow_button), None, false);
                }
                SPEC_NARROW => {
                    state.wideband = false;
                    state.controller.set_wideband(false);
                    let _ = InvalidateRect(Some(state.wide_button), None, false);
                    let _ = InvalidateRect(Some(state.narrow_button), None, false);
                }
                SPEC_FREEZE => {
                    state.frozen = !state.frozen;
                    state.controller.set_frozen(state.frozen);
                    let _ = InvalidateRect(Some(state.freeze_button), None, false);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_DRAWITEM => {
            let item = &*(lparam.0 as *const DRAWITEMSTRUCT);
            if item.hwndItem == state.view && !state.dib.is_invalid() && !state.bits.is_null() {
                let memory = CreateCompatibleDC(Some(item.hDC));
                if !memory.is_invalid() {
                    let previous = SelectObject(memory, HGDIOBJ(state.dib.0));
                    let _ = BitBlt(
                        item.hDC,
                        0,
                        0,
                        IMAGE_WIDTH as i32,
                        IMAGE_HEIGHT as i32,
                        Some(memory),
                        0,
                        0,
                        SRCCOPY,
                    );
                    SelectObject(memory, previous);
                    let _ = DeleteDC(memory);
                }
                return LRESULT(1);
            }
            if item.hwndItem == state.meter_view {
                draw_meter(item, state);
            } else if item.hwndItem == state.wide_button {
                draw_button(item, state, state.wideband);
            } else if item.hwndItem == state.narrow_button {
                draw_button(item, state, !state.wideband);
            } else if item.hwndItem == state.freeze_button {
                draw_button(item, state, state.frozen);
            } else {
                return DefWindowProcW(window, message, wparam, lparam);
            }
            LRESULT(1)
        }
        WM_TIMER => {
            update_image(state);
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(window);
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = KillTimer(Some(window), 1);
            destroy_dib(state);
            if !state.font.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(state.font.0));
                state.font = HFONT::default();
            }
            if !state.bg_brush.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(state.bg_brush.0));
                state.bg_brush = HBRUSH::default();
            }
            SetWindowLongPtrW(window, GWLP_USERDATA, 0);
            set_window_handle(HWND::default());
            drop(Box::from_raw(state_ptr));
            LRESULT(0)
        }
        _ => DefWindowProcW(window, message, wparam, lparam),
    }
}

/// # Safety
/// Call only on the desktop main thread, including all subsequent window operations.
pub unsafe fn open() -> Result<(), String> {
    if is_open() {
        focus();
        return Ok(());
    }
    let controller = SpectrumController::start().map_err(|error| error.to_string())?;
    let font = CreateFontW(
        -16,
        0,
        0,
        0,
        FW_NORMAL.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        CLEARTYPE_QUALITY,
        DEFAULT_PITCH.0 as u32,
        w!("Segoe UI"),
    );
    let bg_brush = CreateSolidBrush(theme::BG);
    if font.is_invalid() || bg_brush.is_invalid() {
        return Err("Cannot open spectrum window: out of GDI resources".into());
    }
    let state = Box::into_raw(Box::new(State {
        controller,
        wide_button: HWND::default(),
        narrow_button: HWND::default(),
        freeze_button: HWND::default(),
        view: HWND::default(),
        meter_view: HWND::default(),
        levels: MeterLevels::default(),
        dib: HBITMAP::default(),
        bits: core::ptr::null_mut(),
        pixels: Vec::new(),
        generation: 0,
        frozen: false,
        wideband: true,
        started: false,
        font,
        bg_brush,
        error: 0,
    }));
    let instance = GetModuleHandleW(None).map_err(|error| error.to_string())?;
    let class_name = w!("RustLowLatencyVoiceChangerSpectrum");
    let class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(spectrum_proc),
        hInstance: instance.into(),
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
        lpszClassName: class_name,
        ..Default::default()
    };
    let atom = RegisterClassW(&class);
    if atom == 0 {
        let error = windows::Win32::Foundation::GetLastError();
        if error != ERROR_CLASS_ALREADY_EXISTS {
            drop(Box::from_raw(state));
            return Err(format!(
                "Cannot open spectrum window: Win32 error {}",
                error.0
            ));
        }
    }
    let mut bounds = RECT {
        left: 0,
        top: 0,
        right: IMAGE_WIDTH as i32 + 192,
        bottom: IMAGE_HEIGHT as i32 + 84,
    };
    let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
    let _ = AdjustWindowRect(&mut bounds, style, false);
    let created = CreateWindowExW(
        Default::default(),
        class_name,
        w!("Formant Spectrogram"),
        style,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        bounds.right - bounds.left,
        bounds.bottom - bounds.top,
        None,
        None,
        Some(instance.into()),
        Some(state as *const _),
    );
    let error = (*state).error;
    let started = (*state).started;
    match created {
        Ok(window) if !window.0.is_null() && error == 0 => {
            let _ = ShowWindow(window, SW_SHOW);
            Ok(())
        }
        _ => {
            let code = if error != 0 {
                error
            } else {
                windows::Win32::Foundation::GetLastError().0
            };
            if !started && window_handle().0.is_null() {
                drop(Box::from_raw(state));
            }
            Err(format!("Cannot open spectrum window: Win32 error {code}"))
        }
    }
}

unsafe fn focus() {
    let window = window_handle();
    if !window.0.is_null() {
        let _ = ShowWindow(window, SW_RESTORE);
        let _ = SetForegroundWindow(window);
    }
}

/// # Safety
/// Call only on the desktop main thread.
pub unsafe fn shutdown() {
    let window = window_handle();
    if !window.0.is_null() {
        let _ = DestroyWindow(window);
    }
}
