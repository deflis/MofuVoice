// Native spectrogram only. All calls and window messages run on Tauri's main thread.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <dwmapi.h>
#include <cstring>
#include <vector>
using SpectrumClose = void (*)(void *);
using SpectrumSetWideband = void (*)(void *, int);
using SpectrumSetFrozen = void (*)(void *, int);
using SpectrumPoll = int (*)(void *, unsigned char *, size_t, unsigned long long *);
enum { SPEC_WIDE = 301, SPEC_NARROW, SPEC_FREEZE };
enum { SPEC_W = 720, SPEC_H = 400, SPEC_BPP = 4 };
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
    void *context = nullptr;
    SpectrumClose spectrum_close = nullptr;
    SpectrumSetWideband spectrum_set_wideband = nullptr;
    SpectrumSetFrozen spectrum_set_frozen = nullptr;
    SpectrumPoll spectrum_poll = nullptr;
    HWND spectrum_window = nullptr;
    HFONT font = nullptr;
    HBRUSH bg_brush = nullptr;
    DWORD error = 0;
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


static void enable_dark_title(HWND window) {
    BOOL dark = TRUE;
    DwmSetWindowAttribute(window, 20, &dark, sizeof(dark));
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
    if (!spec.dib || !spec.bits) return false;
    memset(spec.bits, 0, static_cast<size_t>(SPEC_W) * SPEC_H * SPEC_BPP);
    return true;
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
        if (gui.font) DeleteObject(gui.font);
        if (gui.bg_brush) DeleteObject(gui.bg_brush);
        gui.font = nullptr;
        gui.bg_brush = nullptr;
        SetWindowLongPtrW(window, GWLP_USERDATA, 0);
        return 0;
    }
    return DefWindowProcW(window, message, wp, lp);
}


static Gui gui;
static SpectrumGui spec;
extern "C" int voice_spectrum_is_open() { return gui.spectrum_window != nullptr; }
extern "C" void voice_spectrum_close() {
    if (gui.spectrum_window) DestroyWindow(gui.spectrum_window);
}
// Takes ownership of context on every path; close releases it exactly once.
extern "C" unsigned long voice_spectrum_open(void *context, SpectrumClose close,
    SpectrumSetWideband wide, SpectrumSetFrozen frozen, SpectrumPoll poll) {
    if (gui.spectrum_window) {
        close(context);
        ShowWindow(gui.spectrum_window, SW_RESTORE);
        SetForegroundWindow(gui.spectrum_window);
        return 0;
    }
    gui = Gui{};
    spec = SpectrumGui{};
    gui.context = context;
    gui.spectrum_close = close;
    gui.spectrum_set_wideband = wide;
    gui.spectrum_set_frozen = frozen;
    gui.spectrum_poll = poll;
    gui.font = CreateFontW(-16, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
        DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
        DEFAULT_PITCH, L"Segoe UI");
    gui.bg_brush = CreateSolidBrush(theme::bg);
    HINSTANCE instance = GetModuleHandleW(nullptr);
    const wchar_t *class_name = L"RustLowLatencyVoiceChangerSpectrum";
    WNDCLASSW wc{};
    wc.lpfnWndProc = spectrum_proc;
    wc.hInstance = instance;
    wc.hCursor = LoadCursorW(nullptr, MAKEINTRESOURCEW(32512));
    wc.lpszClassName = class_name;
    if (!gui.font || !gui.bg_brush) gui.error = ERROR_NOT_ENOUGH_MEMORY;
    if (!gui.error && !RegisterClassW(&wc) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS)
        gui.error = GetLastError();
    if (!gui.error) {
        spec.owner = &gui;
        RECT bounds{0, 0, SPEC_W + 32, SPEC_H + 84};
        DWORD style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        AdjustWindowRect(&bounds, style, FALSE);
        gui.spectrum_window = CreateWindowExW(0, class_name, L"Formant Spectrogram", style,
            CW_USEDEFAULT, CW_USEDEFAULT, bounds.right - bounds.left, bounds.bottom - bounds.top,
            nullptr, nullptr, instance, &spec);
        if (!gui.spectrum_window) gui.error = gui.error ? gui.error : ERROR_GEN_FAILURE;
    }
    if (gui.error) {
        if (!spec.started) close(context);
        if (gui.font) DeleteObject(gui.font);
        if (gui.bg_brush) DeleteObject(gui.bg_brush);
        gui.font = nullptr;
        gui.bg_brush = nullptr;
        return gui.error;
    }
    ShowWindow(gui.spectrum_window, SW_SHOW);
    return 0;
}
extern "C" void voice_spectrum_focus() {
    if (gui.spectrum_window) {
        ShowWindow(gui.spectrum_window, SW_RESTORE);
        SetForegroundWindow(gui.spectrum_window);
    }
}
