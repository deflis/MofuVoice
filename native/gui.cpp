// Native controls only. All driver work is delegated to the Rust audio worker.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <commctrl.h>
#include <dwmapi.h>
#include <cmath>
#include <cstring>
#include <string>
#include <vector>

#pragma comment(lib, "comctl32.lib")
#pragma comment(lib, "dwmapi.lib")

using Start = void (*)(void *, const wchar_t *, const wchar_t *, const wchar_t *, const wchar_t *, int, int);
using Stop = void (*)(void *);
using Poll = int (*)(void *, wchar_t *, size_t);
using Save = int (*)(void *, const wchar_t *, const wchar_t *, const wchar_t *, const wchar_t *, int);
using SpectrumOpen = int (*)(void *);
using SpectrumClose = void (*)(void *);
using SpectrumSetWideband = void (*)(void *, int);
using SpectrumSetFrozen = void (*)(void *, int);
using SpectrumPoll = int (*)(void *, unsigned char *, size_t, unsigned long long *);

enum {
    START = 101, STOP, INSPECT, SPECTRUM, SETTINGS, STATUS_OPEN
};
enum { SPEC_WIDE = 301, SPEC_NARROW, SPEC_FREEZE };
enum {
    SET_DEVICE = 201, SET_FRAMES = 202, SET_PITCH = 203, SET_FORMANT = 204,
    SET_SHIFT = 206, SET_PITCH_SLIDER = 207, SET_SHIFT_SLIDER = 208
};
enum { STATUS_DETAIL = 205, STATUS_SUMMARY = 209 };
enum { SPEC_W = 720, SPEC_H = 400, SPEC_BPP = 4 };
enum { SLIDER_MIN = -1200, SLIDER_MAX = 1200 }; // centi-semitones

namespace theme {
constexpr COLORREF bg = RGB(22, 24, 28);
constexpr COLORREF surface = RGB(32, 36, 42);
constexpr COLORREF border = RGB(55, 60, 70);
constexpr COLORREF text = RGB(230, 232, 236);
constexpr COLORREF muted = RGB(140, 148, 160);
constexpr COLORREF accent = RGB(0, 196, 180);
constexpr COLORREF accent_dim = RGB(0, 140, 130);
constexpr COLORREF button = RGB(42, 46, 54);
constexpr COLORREF button_hot = RGB(52, 58, 68);
}

struct Gui {
    void *context;
    Start start;
    Stop stop;
    Poll poll;
    Save save;
    SpectrumOpen spectrum_open;
    SpectrumClose spectrum_close;
    SpectrumSetWideband spectrum_set_wideband;
    SpectrumSetFrozen spectrum_set_frozen;
    SpectrumPoll spectrum_poll;
    const wchar_t *devices;
    std::wstring device_value;
    std::wstring frames_value;
    std::wstring pitch_value;
    std::wstring shift_value;
    int preserve_value = 1;
    HWND main = nullptr;
    HWND settings_window = nullptr;
    HWND status_window = nullptr;
    HWND spectrum_window = nullptr;
    HWND status_summary = nullptr;
    HWND status_detail = nullptr;
    HWND device = nullptr, frames = nullptr, pitch = nullptr, formant = nullptr, shift = nullptr;
    HWND pitch_slider = nullptr, shift_slider = nullptr;
    HWND start_button = nullptr, stop_button = nullptr, inspect_button = nullptr;
    HWND spectrum_button = nullptr, settings_button = nullptr, status_button = nullptr;
    HFONT font = nullptr;
    HFONT font_title = nullptr;
    HFONT font_small = nullptr;
    HBRUSH bg_brush = nullptr;
    HBRUSH surface_brush = nullptr;
    HBRUSH edit_brush = nullptr;
    bool closing = false;
    bool syncing_slider = false;
    bool busy = false;
    DWORD error = 0;
    std::wstring last_status;
};

struct SpectrumGui {
    Gui *owner = nullptr;
    HWND wide_button = nullptr;
    HWND narrow_button = nullptr;
    HWND freeze_button = nullptr;
    HWND view = nullptr;
    HBITMAP dib = nullptr;
    void *bits = nullptr;
    unsigned long long generation = 0;
    bool frozen = false;
    bool wideband = true;
    bool started = false;
    std::vector<unsigned char> pixels;
};

extern "C" void voice_gui_message(const wchar_t *title, const wchar_t *message, int error) {
    MessageBoxW(nullptr, message, title, MB_OK | (error ? MB_ICONERROR : MB_ICONINFORMATION));
}

static void enable_dark_title(HWND window) {
    BOOL dark = TRUE;
    DwmSetWindowAttribute(window, 20, &dark, sizeof(dark));
}

static HWND control(HWND parent, Gui &gui, const wchar_t *type, const wchar_t *text,
                    DWORD style, int x, int y, int width, int height, int id = 0,
                    DWORD ex = 0) {
    HWND child = CreateWindowExW(ex, type, text, WS_CHILD | WS_VISIBLE | style,
        x, y, width, height, parent, reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)),
        GetModuleHandleW(nullptr), nullptr);
    if (!child) gui.error = GetLastError();
    if (child) SendMessageW(child, WM_SETFONT, reinterpret_cast<WPARAM>(gui.font), TRUE);
    return child;
}

static std::wstring text_of(HWND hwnd) {
    if (!hwnd) return {};
    int length = GetWindowTextLengthW(hwnd);
    std::wstring value(static_cast<size_t>(length) + 1, L'\0');
    GetWindowTextW(hwnd, value.data(), length + 1);
    value.resize(static_cast<size_t>(length));
    return value;
}

static std::wstring first_line(const std::wstring &status) {
    size_t end = status.find_first_of(L"\r\n");
    return end == std::wstring::npos ? status : status.substr(0, end);
}

static double parse_semitones(const std::wstring &text, bool &ok) {
    if (text.empty()) { ok = false; return 0; }
    wchar_t *end = nullptr;
    double value = wcstod(text.c_str(), &end);
    ok = end && *end == L'\0' && std::isfinite(value);
    return value;
}

static std::wstring format_semitones(double value) {
    wchar_t buffer[64];
    swprintf_s(buffer, L"%.2f", value);
    std::wstring text = buffer;
    while (text.size() > 1 && text.find(L'.') != std::wstring::npos && text.back() == L'0')
        text.pop_back();
    if (!text.empty() && text.back() == L'.') text.pop_back();
    if (text == L"-0") text = L"0";
    return text;
}

static int semitones_to_pos(double value) {
    if (value < -12.0) value = -12.0;
    if (value > 12.0) value = 12.0;
    return static_cast<int>(std::lround(value * 100.0));
}

static double pos_to_semitones(int pos) {
    if (pos < SLIDER_MIN) pos = SLIDER_MIN;
    if (pos > SLIDER_MAX) pos = SLIDER_MAX;
    return pos / 100.0;
}

static void pull_settings_from_controls(Gui &gui) {
    if (!gui.settings_window) return;
    gui.device_value = text_of(gui.device);
    gui.frames_value = text_of(gui.frames);
    gui.pitch_value = text_of(gui.pitch);
    gui.shift_value = text_of(gui.shift);
    if (gui.formant)
        gui.preserve_value = SendMessageW(gui.formant, BM_GETCHECK, 0, 0) == BST_CHECKED;
}

static void set_settings_enabled(Gui &gui, bool enabled) {
    if (!gui.settings_window) return;
    for (HWND item : {gui.device, gui.frames, gui.pitch, gui.formant, gui.shift,
                      gui.pitch_slider, gui.shift_slider}) {
        if (item) EnableWindow(item, enabled);
    }
}

static void sync_slider_from_edit(Gui &gui, HWND edit, HWND slider) {
    if (!edit || !slider || gui.syncing_slider) return;
    bool ok = false;
    double value = parse_semitones(text_of(edit), ok);
    if (!ok) return;
    gui.syncing_slider = true;
    SendMessageW(slider, TBM_SETPOS, TRUE, semitones_to_pos(value));
    gui.syncing_slider = false;
}

static void sync_edit_from_slider(Gui &gui, HWND edit, HWND slider) {
    if (!edit || !slider || gui.syncing_slider) return;
    gui.syncing_slider = true;
    SetWindowTextW(edit, format_semitones(pos_to_semitones(
        static_cast<int>(SendMessageW(slider, TBM_GETPOS, 0, 0)))).c_str());
    gui.syncing_slider = false;
}

static void paint_background(HWND window, HDC dc, HBRUSH brush) {
    RECT rect{};
    GetClientRect(window, &rect);
    FillRect(dc, &rect, brush);
}

static void draw_label(HDC dc, HFONT font, COLORREF color, int x, int y, int w, int h,
                       const wchar_t *text, UINT format = DT_LEFT | DT_VCENTER | DT_SINGLELINE) {
    RECT rect{x, y, x + w, y + h};
    HGDIOBJ previous = SelectObject(dc, font);
    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, color);
    DrawTextW(dc, text, -1, &rect, format);
    SelectObject(dc, previous);
}

static void draw_button(const DRAWITEMSTRUCT *item, Gui &gui, bool primary) {
    const bool disabled = (item->itemState & ODS_DISABLED) != 0;
    const bool hot = (item->itemState & ODS_HOTLIGHT) != 0 || (item->itemState & ODS_SELECTED) != 0;
    COLORREF fill = primary
        ? (disabled ? theme::accent_dim : (hot ? RGB(0, 214, 196) : theme::accent))
        : (disabled ? RGB(36, 40, 46) : (hot ? theme::button_hot : theme::button));
    COLORREF text = primary
        ? (disabled ? RGB(180, 220, 215) : RGB(10, 20, 22))
        : (disabled ? theme::muted : theme::text);
    HBRUSH brush = CreateSolidBrush(fill);
    HPEN pen = CreatePen(PS_SOLID, 1, primary ? fill : theme::border);
    HGDIOBJ old_brush = SelectObject(item->hDC, brush);
    HGDIOBJ old_pen = SelectObject(item->hDC, pen);
    RoundRect(item->hDC, item->rcItem.left, item->rcItem.top,
        item->rcItem.right, item->rcItem.bottom, 10, 10);
    SelectObject(item->hDC, old_brush);
    SelectObject(item->hDC, old_pen);
    DeleteObject(brush);
    DeleteObject(pen);
    wchar_t label[64] = {};
    GetWindowTextW(item->hwndItem, label, 64);
    draw_label(item->hDC, gui.font, text, item->rcItem.left, item->rcItem.top,
        item->rcItem.right - item->rcItem.left, item->rcItem.bottom - item->rcItem.top,
        label, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
}

static void refresh(HWND window, Gui &gui) {
    wchar_t status[8192] = {};
    gui.busy = gui.poll(gui.context, status, 8192) != 0;
    if (gui.last_status != status) {
        gui.last_status = status;
        std::wstring summary = first_line(gui.last_status);
        if (gui.status_summary) SetWindowTextW(gui.status_summary, summary.c_str());
        if (gui.status_detail) SetWindowTextW(gui.status_detail, status);
    }
    const bool editable = !gui.busy && !gui.closing;
    for (HWND item : {gui.start_button, gui.inspect_button, gui.settings_button}) {
        if (item) EnableWindow(item, editable || item == gui.settings_button);
    }
    if (gui.settings_button) EnableWindow(gui.settings_button, !gui.closing);
    if (gui.status_button) EnableWindow(gui.status_button, !gui.closing);
    if (gui.spectrum_button) EnableWindow(gui.spectrum_button, !gui.closing);
    if (gui.stop_button) EnableWindow(gui.stop_button, gui.busy && !gui.closing);
    if (gui.start_button) EnableWindow(gui.start_button, editable);
    if (gui.inspect_button) EnableWindow(gui.inspect_button, editable);
    set_settings_enabled(gui, editable);
    if (gui.closing && !gui.busy) DestroyWindow(window);
}

static void destroy_spectrum_resources(SpectrumGui &spec) {
    if (spec.dib) {
        DeleteObject(spec.dib);
        spec.dib = nullptr;
        spec.bits = nullptr;
    }
}

static bool ensure_spectrum_dib(SpectrumGui &spec) {
    if (spec.dib) return true;
    BITMAPINFO info{};
    info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    info.bmiHeader.biWidth = SPEC_W;
    info.bmiHeader.biHeight = -SPEC_H;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    HDC screen = GetDC(nullptr);
    if (!screen) return false;
    spec.dib = CreateDIBSection(screen, &info, DIB_RGB_COLORS, &spec.bits, nullptr, 0);
    ReleaseDC(nullptr, screen);
    return spec.dib && spec.bits;
}

static void update_spectrum_image(SpectrumGui &spec) {
    Gui &gui = *spec.owner;
    if (!ensure_spectrum_dib(spec)) {
        gui.error = GetLastError();
        return;
    }
    const size_t bytes = static_cast<size_t>(SPEC_W) * SPEC_H * SPEC_BPP;
    if (spec.pixels.size() != bytes) spec.pixels.assign(bytes, 0);
    if (gui.spectrum_poll(gui.context, spec.pixels.data(), spec.pixels.size(), &spec.generation)) {
        memcpy(spec.bits, spec.pixels.data(), bytes);
        InvalidateRect(spec.view, nullptr, FALSE);
    }
}

static LRESULT CALLBACK spectrum_proc(HWND window, UINT message, WPARAM wp, LPARAM lp) {
    auto *spec = reinterpret_cast<SpectrumGui *>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
        spec = static_cast<SpectrumGui *>(reinterpret_cast<CREATESTRUCTW *>(lp)->lpCreateParams);
        SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(spec));
    }
    if (!spec) return DefWindowProcW(window, message, wp, lp);
    Gui &gui = *spec->owner;
    switch (message) {
    case WM_CREATE: {
        enable_dark_title(window);
        spec->wide_button = CreateWindowExW(0, L"BUTTON", L"Wideband",
            WS_CHILD | WS_VISIBLE | BS_OWNERDRAW | WS_GROUP | WS_TABSTOP,
            16, 16, 110, 32, window, reinterpret_cast<HMENU>(SPEC_WIDE),
            GetModuleHandleW(nullptr), nullptr);
        spec->narrow_button = CreateWindowExW(0, L"BUTTON", L"Narrowband",
            WS_CHILD | WS_VISIBLE | BS_OWNERDRAW | WS_TABSTOP,
            136, 16, 120, 32, window, reinterpret_cast<HMENU>(SPEC_NARROW),
            GetModuleHandleW(nullptr), nullptr);
        spec->freeze_button = CreateWindowExW(0, L"BUTTON", L"Freeze",
            WS_CHILD | WS_VISIBLE | BS_OWNERDRAW | WS_TABSTOP,
            266, 16, 100, 32, window, reinterpret_cast<HMENU>(SPEC_FREEZE),
            GetModuleHandleW(nullptr), nullptr);
        spec->view = CreateWindowExW(0, L"STATIC", L"",
            WS_CHILD | WS_VISIBLE | SS_OWNERDRAW,
            16, 60, SPEC_W, SPEC_H, window, nullptr, GetModuleHandleW(nullptr), nullptr);
        for (HWND item : {spec->wide_button, spec->narrow_button, spec->freeze_button}) {
            if (!item) { gui.error = GetLastError(); return -1; }
            SendMessageW(item, WM_SETFONT, reinterpret_cast<WPARAM>(gui.font), TRUE);
        }
        if (!spec->view) { gui.error = GetLastError(); return -1; }
        if (!gui.spectrum_open(gui.context)) return -1;
        spec->started = true;
        gui.spectrum_set_wideband(gui.context, 1);
        if (!ensure_spectrum_dib(*spec)) { gui.error = GetLastError(); return -1; }
        if (!SetTimer(window, 1, 33, nullptr)) { gui.error = GetLastError(); return -1; }
        return 0;
    }
    case WM_ERASEBKGND:
        paint_background(window, reinterpret_cast<HDC>(wp), gui.bg_brush);
        return 1;
    case WM_COMMAND:
        if (HIWORD(wp) != BN_CLICKED) break;
        if (LOWORD(wp) == SPEC_WIDE) {
            spec->wideband = true;
            gui.spectrum_set_wideband(gui.context, 1);
            InvalidateRect(spec->wide_button, nullptr, FALSE);
            InvalidateRect(spec->narrow_button, nullptr, FALSE);
        } else if (LOWORD(wp) == SPEC_NARROW) {
            spec->wideband = false;
            gui.spectrum_set_wideband(gui.context, 0);
            InvalidateRect(spec->wide_button, nullptr, FALSE);
            InvalidateRect(spec->narrow_button, nullptr, FALSE);
        } else if (LOWORD(wp) == SPEC_FREEZE) {
            spec->frozen = !spec->frozen;
            gui.spectrum_set_frozen(gui.context, spec->frozen ? 1 : 0);
            InvalidateRect(spec->freeze_button, nullptr, FALSE);
        }
        return 0;
    case WM_DRAWITEM: {
        auto *item = reinterpret_cast<DRAWITEMSTRUCT *>(lp);
        if (!item) break;
        if (item->hwndItem == spec->view && spec->dib && spec->bits) {
            HDC memory = CreateCompatibleDC(item->hDC);
            if (memory) {
                HGDIOBJ previous = SelectObject(memory, spec->dib);
                BitBlt(item->hDC, 0, 0, SPEC_W, SPEC_H, memory, 0, 0, SRCCOPY);
                SelectObject(memory, previous);
                DeleteDC(memory);
            }
            return TRUE;
        }
        if (item->hwndItem == spec->wide_button)
            draw_button(item, gui, spec->wideband);
        else if (item->hwndItem == spec->narrow_button)
            draw_button(item, gui, !spec->wideband);
        else if (item->hwndItem == spec->freeze_button)
            draw_button(item, gui, spec->frozen);
        else
            break;
        return TRUE;
    }
    case WM_TIMER:
        update_spectrum_image(*spec);
        return 0;
    case WM_CLOSE:
        DestroyWindow(window);
        return 0;
    case WM_DESTROY:
        KillTimer(window, 1);
        if (spec->started) gui.spectrum_close(gui.context);
        destroy_spectrum_resources(*spec);
        gui.spectrum_window = nullptr;
        delete spec;
        return 0;
    }
    return DefWindowProcW(window, message, wp, lp);
}

static void open_spectrum_window(HWND parent, Gui &gui) {
    if (gui.spectrum_window) {
        SetForegroundWindow(gui.spectrum_window);
        return;
    }
    HINSTANCE instance = GetModuleHandleW(nullptr);
    const wchar_t *class_name = L"RustLowLatencyVoiceChangerSpectrum";
    WNDCLASSW wc{};
    wc.lpfnWndProc = spectrum_proc;
    wc.hInstance = instance;
    wc.hCursor = LoadCursorW(nullptr, MAKEINTRESOURCEW(32512));
    wc.hbrBackground = gui.bg_brush;
    wc.lpszClassName = class_name;
    if (!RegisterClassW(&wc) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) {
        gui.error = GetLastError();
        return;
    }
    auto *spec = new SpectrumGui{};
    spec->owner = &gui;
    RECT bounds{0, 0, SPEC_W + 32, SPEC_H + 84};
    DWORD style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
    AdjustWindowRect(&bounds, style, FALSE);
    gui.spectrum_window = CreateWindowExW(0, class_name, L"Formant Spectrogram", style,
        CW_USEDEFAULT, CW_USEDEFAULT, bounds.right - bounds.left, bounds.bottom - bounds.top,
        parent, nullptr, instance, spec);
    if (!gui.spectrum_window) {
        gui.error = gui.error ? gui.error : GetLastError();
        delete spec;
        return;
    }
    ShowWindow(gui.spectrum_window, SW_SHOW);
}

static LRESULT CALLBACK status_proc(HWND window, UINT message, WPARAM wp, LPARAM lp) {
    auto *gui = reinterpret_cast<Gui *>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
        gui = static_cast<Gui *>(reinterpret_cast<CREATESTRUCTW *>(lp)->lpCreateParams);
        SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(gui));
    }
    if (!gui) return DefWindowProcW(window, message, wp, lp);
    switch (message) {
    case WM_CREATE: {
        enable_dark_title(window);
        gui->status_detail = CreateWindowExW(WS_EX_CLIENTEDGE, L"EDIT", gui->last_status.c_str(),
            WS_CHILD | WS_VISIBLE | ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL | WS_VSCROLL | WS_TABSTOP,
            16, 16, 560, 420, window, reinterpret_cast<HMENU>(STATUS_DETAIL),
            GetModuleHandleW(nullptr), nullptr);
        if (!gui->status_detail) { gui->error = GetLastError(); return -1; }
        SendMessageW(gui->status_detail, WM_SETFONT, reinterpret_cast<WPARAM>(gui->font_small), TRUE);
        return 0;
    }
    case WM_SIZE: {
        if (gui->status_detail) {
            RECT rect{};
            GetClientRect(window, &rect);
            MoveWindow(gui->status_detail, 16, 16, rect.right - 32, rect.bottom - 32, TRUE);
        }
        return 0;
    }
    case WM_CTLCOLOREDIT:
    case WM_CTLCOLORSTATIC: {
        HDC dc = reinterpret_cast<HDC>(wp);
        SetTextColor(dc, theme::text);
        SetBkColor(dc, theme::surface);
        return reinterpret_cast<LRESULT>(gui->edit_brush);
    }
    case WM_ERASEBKGND:
        paint_background(window, reinterpret_cast<HDC>(wp), gui->bg_brush);
        return 1;
    case WM_CLOSE:
        DestroyWindow(window);
        return 0;
    case WM_DESTROY:
        gui->status_detail = nullptr;
        gui->status_window = nullptr;
        return 0;
    }
    return DefWindowProcW(window, message, wp, lp);
}

static void open_status_window(HWND parent, Gui &gui) {
    if (gui.status_window) {
        SetForegroundWindow(gui.status_window);
        return;
    }
    HINSTANCE instance = GetModuleHandleW(nullptr);
    const wchar_t *class_name = L"RustLowLatencyVoiceChangerStatus";
    WNDCLASSW wc{};
    wc.lpfnWndProc = status_proc;
    wc.hInstance = instance;
    wc.hCursor = LoadCursorW(nullptr, MAKEINTRESOURCEW(32512));
    wc.hbrBackground = gui.bg_brush;
    wc.lpszClassName = class_name;
    if (!RegisterClassW(&wc) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) {
        gui.error = GetLastError();
        return;
    }
    RECT bounds{0, 0, 592, 452};
    DWORD style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_THICKFRAME;
    AdjustWindowRect(&bounds, style, FALSE);
    gui.status_window = CreateWindowExW(0, class_name, L"Status", style,
        CW_USEDEFAULT, CW_USEDEFAULT, bounds.right - bounds.left, bounds.bottom - bounds.top,
        parent, nullptr, instance, &gui);
    if (!gui.status_window) {
        gui.error = gui.error ? gui.error : GetLastError();
        return;
    }
    ShowWindow(gui.status_window, SW_SHOW);
}

static void setup_slider(HWND slider, int pos) {
    SendMessageW(slider, TBM_SETRANGEMIN, FALSE, SLIDER_MIN);
    SendMessageW(slider, TBM_SETRANGEMAX, TRUE, SLIDER_MAX);
    SendMessageW(slider, TBM_SETPAGESIZE, 0, 100);
    SendMessageW(slider, TBM_SETTICFREQ, 100, 0);
    SendMessageW(slider, TBM_SETPOS, TRUE, pos);
}

static LRESULT CALLBACK settings_proc(HWND window, UINT message, WPARAM wp, LPARAM lp) {
    auto *gui = reinterpret_cast<Gui *>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
        gui = static_cast<Gui *>(reinterpret_cast<CREATESTRUCTW *>(lp)->lpCreateParams);
        SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(gui));
    }
    if (!gui) return DefWindowProcW(window, message, wp, lp);
    switch (message) {
    case WM_CREATE: {
        enable_dark_title(window);
        gui->device = control(window, *gui, L"COMBOBOX", L"",
            CBS_DROPDOWN | CBS_AUTOHSCROLL | WS_VSCROLL | WS_TABSTOP,
            140, 52, 360, 240, SET_DEVICE);
        for (auto name = gui->devices; *name; name += wcslen(name) + 1)
            SendMessageW(gui->device, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(name));
        SetWindowTextW(gui->device, gui->device_value.c_str());
        SendMessageW(gui->device, CB_LIMITTEXT, 512, 0);

        gui->frames = control(window, *gui, L"EDIT", gui->frames_value.c_str(),
            ES_AUTOHSCROLL | WS_TABSTOP, 140, 100, 100, 28, SET_FRAMES, WS_EX_CLIENTEDGE);
        SendMessageW(gui->frames, EM_SETLIMITTEXT, 16, 0);

        gui->pitch_slider = CreateWindowExW(0, TRACKBAR_CLASSW, L"",
            WS_CHILD | WS_VISIBLE | TBS_AUTOTICKS | TBS_BOTH | WS_TABSTOP,
            140, 148, 260, 36, window, reinterpret_cast<HMENU>(SET_PITCH_SLIDER),
            GetModuleHandleW(nullptr), nullptr);
        gui->pitch = control(window, *gui, L"EDIT", gui->pitch_value.c_str(),
            ES_AUTOHSCROLL | WS_TABSTOP, 416, 152, 84, 28, SET_PITCH, WS_EX_CLIENTEDGE);
        SendMessageW(gui->pitch, EM_SETLIMITTEXT, 32, 0);

        gui->formant = control(window, *gui, L"BUTTON", L"Preserve formants",
            BS_AUTOCHECKBOX | WS_TABSTOP, 140, 200, 240, 28, SET_FORMANT);
        SendMessageW(gui->formant, BM_SETCHECK,
            gui->preserve_value ? BST_CHECKED : BST_UNCHECKED, 0);

        gui->shift_slider = CreateWindowExW(0, TRACKBAR_CLASSW, L"",
            WS_CHILD | WS_VISIBLE | TBS_AUTOTICKS | TBS_BOTH | WS_TABSTOP,
            140, 244, 260, 36, window, reinterpret_cast<HMENU>(SET_SHIFT_SLIDER),
            GetModuleHandleW(nullptr), nullptr);
        gui->shift = control(window, *gui, L"EDIT", gui->shift_value.c_str(),
            ES_AUTOHSCROLL | WS_TABSTOP, 416, 248, 84, 28, SET_SHIFT, WS_EX_CLIENTEDGE);
        SendMessageW(gui->shift, EM_SETLIMITTEXT, 32, 0);

        if (!gui->device || !gui->frames || !gui->pitch || !gui->formant || !gui->shift
            || !gui->pitch_slider || !gui->shift_slider) {
            if (!gui->error) gui->error = GetLastError();
            return -1;
        }
        bool ok = false;
        setup_slider(gui->pitch_slider, semitones_to_pos(parse_semitones(gui->pitch_value, ok)));
        setup_slider(gui->shift_slider, semitones_to_pos(parse_semitones(gui->shift_value, ok)));
        set_settings_enabled(*gui, !gui->busy && !gui->closing);
        return 0;
    }
    case WM_PAINT: {
        PAINTSTRUCT ps{};
        HDC dc = BeginPaint(window, &ps);
        paint_background(window, dc, gui->bg_brush);
        draw_label(dc, gui->font_title, theme::text, 20, 16, 480, 28, L"Settings");
        draw_label(dc, gui->font, theme::muted, 20, 52, 110, 28, L"Device");
        draw_label(dc, gui->font, theme::muted, 20, 100, 110, 28, L"Frames");
        draw_label(dc, gui->font_small, theme::muted, 260, 100, 260, 28,
            L"48,000 Hz | Input 1 -> output 1");
        draw_label(dc, gui->font, theme::muted, 20, 152, 110, 28, L"Pitch");
        draw_label(dc, gui->font_small, theme::muted, 20, 184, 480, 20,
            L"Slider and value: -12 to +12 semitones");
        draw_label(dc, gui->font, theme::muted, 20, 248, 110, 28, L"Formant shift");
        draw_label(dc, gui->font_small, theme::muted, 20, 288, 480, 40,
            L"Stop before changing settings. Values stay until the next Start.");
        EndPaint(window, &ps);
        return 0;
    }
    case WM_ERASEBKGND:
        return 1;
    case WM_CTLCOLOREDIT: {
        HDC dc = reinterpret_cast<HDC>(wp);
        SetTextColor(dc, theme::text);
        SetBkColor(dc, theme::surface);
        return reinterpret_cast<LRESULT>(gui->edit_brush);
    }
    case WM_CTLCOLORSTATIC: {
        HDC dc = reinterpret_cast<HDC>(wp);
        SetTextColor(dc, theme::text);
        SetBkColor(dc, theme::bg);
        return reinterpret_cast<LRESULT>(gui->bg_brush);
    }
    case WM_CTLCOLORLISTBOX: {
        HDC dc = reinterpret_cast<HDC>(wp);
        SetTextColor(dc, theme::text);
        SetBkColor(dc, theme::surface);
        return reinterpret_cast<LRESULT>(gui->edit_brush);
    }
    case WM_CTLCOLORBTN: {
        HDC dc = reinterpret_cast<HDC>(wp);
        SetTextColor(dc, theme::text);
        SetBkColor(dc, theme::bg);
        return reinterpret_cast<LRESULT>(gui->bg_brush);
    }
    case WM_HSCROLL:
        if (reinterpret_cast<HWND>(lp) == gui->pitch_slider)
            sync_edit_from_slider(*gui, gui->pitch, gui->pitch_slider);
        else if (reinterpret_cast<HWND>(lp) == gui->shift_slider)
            sync_edit_from_slider(*gui, gui->shift, gui->shift_slider);
        pull_settings_from_controls(*gui);
        return 0;
    case WM_COMMAND: {
        const int id = LOWORD(wp);
        const int code = HIWORD(wp);
        if ((id == SET_PITCH || id == SET_SHIFT) && (code == EN_CHANGE || code == EN_KILLFOCUS)) {
            if (id == SET_PITCH) sync_slider_from_edit(*gui, gui->pitch, gui->pitch_slider);
            else sync_slider_from_edit(*gui, gui->shift, gui->shift_slider);
            pull_settings_from_controls(*gui);
        } else if (id == SET_DEVICE || id == SET_FRAMES || id == SET_FORMANT) {
            pull_settings_from_controls(*gui);
        }
        return 0;
    }
    case WM_CLOSE:
        pull_settings_from_controls(*gui);
        DestroyWindow(window);
        return 0;
    case WM_DESTROY:
        pull_settings_from_controls(*gui);
        gui->device = gui->frames = gui->pitch = gui->formant = gui->shift = nullptr;
        gui->pitch_slider = gui->shift_slider = nullptr;
        gui->settings_window = nullptr;
        return 0;
    }
    return DefWindowProcW(window, message, wp, lp);
}

static void open_settings_window(HWND parent, Gui &gui) {
    if (gui.settings_window) {
        SetForegroundWindow(gui.settings_window);
        return;
    }
    HINSTANCE instance = GetModuleHandleW(nullptr);
    const wchar_t *class_name = L"RustLowLatencyVoiceChangerSettings";
    WNDCLASSW wc{};
    wc.lpfnWndProc = settings_proc;
    wc.hInstance = instance;
    wc.hCursor = LoadCursorW(nullptr, MAKEINTRESOURCEW(32512));
    wc.hbrBackground = gui.bg_brush;
    wc.lpszClassName = class_name;
    if (!RegisterClassW(&wc) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) {
        gui.error = GetLastError();
        return;
    }
    RECT bounds{0, 0, 520, 360};
    DWORD style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
    AdjustWindowRect(&bounds, style, FALSE);
    gui.settings_window = CreateWindowExW(0, class_name, L"Settings", style,
        CW_USEDEFAULT, CW_USEDEFAULT, bounds.right - bounds.left, bounds.bottom - bounds.top,
        parent, nullptr, instance, &gui);
    if (!gui.settings_window) {
        gui.error = gui.error ? gui.error : GetLastError();
        return;
    }
    ShowWindow(gui.settings_window, SW_SHOW);
}

static LRESULT CALLBACK window_proc(HWND window, UINT message, WPARAM wp, LPARAM lp) {
    auto *gui = reinterpret_cast<Gui *>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
        gui = static_cast<Gui *>(reinterpret_cast<CREATESTRUCTW *>(lp)->lpCreateParams);
        SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(gui));
    }
    if (!gui) return DefWindowProcW(window, message, wp, lp);
    switch (message) {
    case WM_CREATE: {
        enable_dark_title(window);
        gui->main = window;
        gui->font = CreateFontW(-16, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
            DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY, DEFAULT_PITCH, L"Segoe UI");
        gui->font_title = CreateFontW(-22, 0, 0, 0, FW_SEMIBOLD, FALSE, FALSE, FALSE,
            DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY, DEFAULT_PITCH, L"Segoe UI");
        gui->font_small = CreateFontW(-14, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
            DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY, DEFAULT_PITCH, L"Segoe UI");
        if (!gui->font || !gui->font_title || !gui->font_small) {
            gui->error = GetLastError();
            return -1;
        }
        gui->status_summary = control(window, *gui, L"STATIC", L"",
            SS_LEFT | SS_NOPREFIX, 24, 72, 432, 28, STATUS_SUMMARY);
        gui->start_button = control(window, *gui, L"BUTTON", L"Start",
            BS_OWNERDRAW | WS_TABSTOP, 24, 120, 130, 40, START);
        gui->stop_button = control(window, *gui, L"BUTTON", L"Stop",
            BS_OWNERDRAW | WS_TABSTOP, 168, 120, 130, 40, STOP);
        gui->inspect_button = control(window, *gui, L"BUTTON", L"Inspect",
            BS_OWNERDRAW | WS_TABSTOP, 312, 120, 130, 40, INSPECT);
        gui->settings_button = control(window, *gui, L"BUTTON", L"Settings",
            BS_OWNERDRAW | WS_TABSTOP, 24, 176, 130, 36, SETTINGS);
        gui->status_button = control(window, *gui, L"BUTTON", L"Status",
            BS_OWNERDRAW | WS_TABSTOP, 168, 176, 130, 36, STATUS_OPEN);
        gui->spectrum_button = control(window, *gui, L"BUTTON", L"Spectrum",
            BS_OWNERDRAW | WS_TABSTOP, 312, 176, 130, 36, SPECTRUM);
        if (gui->error) return -1;
        if (!SetTimer(window, 1, 100, nullptr)) { gui->error = GetLastError(); return -1; }
        refresh(window, *gui);
        return 0;
    }
    case WM_PAINT: {
        PAINTSTRUCT ps{};
        HDC dc = BeginPaint(window, &ps);
        paint_background(window, dc, gui->bg_brush);
        draw_label(dc, gui->font_title, theme::text, 24, 20, 420, 32,
            L"Voice Changer");
        draw_label(dc, gui->font_small, theme::muted, 24, 52, 420, 20,
            L"Monitor and control playback");
        HBRUSH panel = CreateSolidBrush(theme::surface);
        HPEN pen = CreatePen(PS_SOLID, 1, theme::border);
        HGDIOBJ old_brush = SelectObject(dc, panel);
        HGDIOBJ old_pen = SelectObject(dc, pen);
        RoundRect(dc, 16, 64, 464, 108, 12, 12);
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(panel);
        DeleteObject(pen);
        draw_label(dc, gui->font_small, theme::muted, 24, 224, 420, 40,
            L"Open Settings to choose device and voice parameters.");
        EndPaint(window, &ps);
        return 0;
    }
    case WM_ERASEBKGND:
        return 1;
    case WM_CTLCOLORSTATIC: {
        HDC dc = reinterpret_cast<HDC>(wp);
        if (reinterpret_cast<HWND>(lp) == gui->status_summary) {
            SetTextColor(dc, theme::text);
            SetBkColor(dc, theme::surface);
            return reinterpret_cast<LRESULT>(gui->surface_brush);
        }
        SetTextColor(dc, theme::muted);
        SetBkColor(dc, theme::bg);
        return reinterpret_cast<LRESULT>(gui->bg_brush);
    }
    case WM_DRAWITEM: {
        auto *item = reinterpret_cast<DRAWITEMSTRUCT *>(lp);
        if (!item) break;
        const bool primary = item->hwndItem == gui->start_button;
        draw_button(item, *gui, primary);
        return TRUE;
    }
    case WM_COMMAND:
        if (HIWORD(wp) != BN_CLICKED || gui->closing) break;
        if (LOWORD(wp) == START || LOWORD(wp) == INSPECT) {
            pull_settings_from_controls(*gui);
            gui->start(gui->context, gui->device_value.c_str(), gui->frames_value.c_str(),
                gui->pitch_value.c_str(), gui->shift_value.c_str(),
                gui->preserve_value, LOWORD(wp) == INSPECT);
            refresh(window, *gui);
        } else if (LOWORD(wp) == STOP) {
            gui->stop(gui->context);
            EnableWindow(gui->stop_button, FALSE);
        } else if (LOWORD(wp) == SPECTRUM) {
            open_spectrum_window(window, *gui);
        } else if (LOWORD(wp) == SETTINGS) {
            open_settings_window(window, *gui);
        } else if (LOWORD(wp) == STATUS_OPEN) {
            open_status_window(window, *gui);
        }
        return 0;
    case WM_TIMER:
        refresh(window, *gui);
        return 0;
    case WM_CLOSE:
        if (gui->closing) return 0;
        pull_settings_from_controls(*gui);
        if (!gui->save(gui->context, gui->device_value.c_str(), gui->frames_value.c_str(),
            gui->pitch_value.c_str(), gui->shift_value.c_str(), gui->preserve_value)) return 0;
        if (gui->spectrum_window) DestroyWindow(gui->spectrum_window);
        if (gui->settings_window) DestroyWindow(gui->settings_window);
        if (gui->status_window) DestroyWindow(gui->status_window);
        gui->closing = true;
        gui->stop(gui->context);
        SetWindowTextW(window, L"Voice Changer - stopping...");
        refresh(window, *gui);
        return 0;
    case WM_DESTROY:
        KillTimer(window, 1);
        PostQuitMessage(0);
        return 0;
    }
    return DefWindowProcW(window, message, wp, lp);
}

extern "C" unsigned long voice_gui_run(void *context, const wchar_t *device,
    const wchar_t *devices, const wchar_t *frames, const wchar_t *pitch, const wchar_t *shift, int preserve,
    Save save, Start start, Stop stop, Poll poll,
    SpectrumOpen spectrum_open, SpectrumClose spectrum_close,
    SpectrumSetWideband spectrum_set_wideband, SpectrumSetFrozen spectrum_set_frozen,
    SpectrumPoll spectrum_poll) {
    INITCOMMONCONTROLSEX ice{sizeof(ice), ICC_BAR_CLASSES | ICC_STANDARD_CLASSES};
    InitCommonControlsEx(&ice);

    Gui gui{};
    gui.context = context;
    gui.start = start;
    gui.stop = stop;
    gui.poll = poll;
    gui.save = save;
    gui.spectrum_open = spectrum_open;
    gui.spectrum_close = spectrum_close;
    gui.spectrum_set_wideband = spectrum_set_wideband;
    gui.spectrum_set_frozen = spectrum_set_frozen;
    gui.spectrum_poll = spectrum_poll;
    gui.devices = devices;
    gui.device_value = device ? device : L"";
    gui.frames_value = frames ? frames : L"";
    gui.pitch_value = pitch ? pitch : L"";
    gui.shift_value = shift ? shift : L"";
    gui.preserve_value = preserve;
    gui.bg_brush = CreateSolidBrush(theme::bg);
    gui.surface_brush = CreateSolidBrush(theme::surface);
    gui.edit_brush = CreateSolidBrush(theme::surface);
    if (!gui.bg_brush || !gui.surface_brush || !gui.edit_brush)
        return GetLastError() ? GetLastError() : ERROR_NOT_ENOUGH_MEMORY;

    HINSTANCE instance = GetModuleHandleW(nullptr);
    const wchar_t *class_name = L"RustLowLatencyVoiceChangerWindow";
    WNDCLASSW wc{};
    wc.lpfnWndProc = window_proc;
    wc.hInstance = instance;
    wc.hCursor = LoadCursorW(nullptr, MAKEINTRESOURCEW(32512));
    wc.hbrBackground = gui.bg_brush;
    wc.lpszClassName = class_name;
    if (!RegisterClassW(&wc)) {
        DWORD error = GetLastError();
        DeleteObject(gui.bg_brush);
        DeleteObject(gui.surface_brush);
        DeleteObject(gui.edit_brush);
        return error;
    }
    RECT bounds{0, 0, 480, 280};
    DWORD style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
    AdjustWindowRect(&bounds, style, FALSE);
    HWND window = CreateWindowExW(0, class_name, L"Voice Changer", style,
        CW_USEDEFAULT, CW_USEDEFAULT, bounds.right - bounds.left, bounds.bottom - bounds.top,
        nullptr, nullptr, instance, &gui);
    DWORD error = gui.error;
    if (!window && !error) error = GetLastError();
    if (window) {
        ShowWindow(window, SW_SHOW);
        MSG msg{};
        int result;
        while ((result = GetMessageW(&msg, nullptr, 0, 0)) > 0) {
            bool handled = (gui.spectrum_window && IsDialogMessageW(gui.spectrum_window, &msg))
                || (gui.settings_window && IsDialogMessageW(gui.settings_window, &msg))
                || (gui.status_window && IsDialogMessageW(gui.status_window, &msg))
                || IsDialogMessageW(window, &msg);
            if (!handled) {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if (result == -1) error = GetLastError();
        if (IsWindow(gui.spectrum_window)) DestroyWindow(gui.spectrum_window);
        if (IsWindow(gui.settings_window)) DestroyWindow(gui.settings_window);
        if (IsWindow(gui.status_window)) DestroyWindow(gui.status_window);
        if (IsWindow(window)) DestroyWindow(window);
    }
    if (gui.font) DeleteObject(gui.font);
    if (gui.font_title) DeleteObject(gui.font_title);
    if (gui.font_small) DeleteObject(gui.font_small);
    DeleteObject(gui.bg_brush);
    DeleteObject(gui.surface_brush);
    DeleteObject(gui.edit_brush);
    UnregisterClassW(class_name, instance);
    UnregisterClassW(L"RustLowLatencyVoiceChangerSpectrum", instance);
    UnregisterClassW(L"RustLowLatencyVoiceChangerSettings", instance);
    UnregisterClassW(L"RustLowLatencyVoiceChangerStatus", instance);
    return error;
}
