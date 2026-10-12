// Native controls only. All driver work is delegated to the Rust audio worker.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <cstring>
#include <string>
#include <vector>

using Start = void (*)(void *, const wchar_t *, const wchar_t *, const wchar_t *, const wchar_t *, int, int);
using Stop = void (*)(void *);
using Poll = int (*)(void *, wchar_t *, size_t);
using Save = int (*)(void *, const wchar_t *, const wchar_t *, const wchar_t *, const wchar_t *, int);
using SpectrumOpen = int (*)(void *);
using SpectrumClose = void (*)(void *);
using SpectrumSetWideband = void (*)(void *, int);
using SpectrumSetFrozen = void (*)(void *, int);
using SpectrumPoll = int (*)(void *, unsigned char *, size_t, unsigned long long *);

enum { START = 101, STOP, INSPECT, SPECTRUM };
enum { SPEC_WIDE = 301, SPEC_NARROW, SPEC_FREEZE };
enum { SPEC_W = 720, SPEC_H = 400, SPEC_BPP = 4 };

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
    const wchar_t *device_text, *devices, *frames_text, *pitch_text, *shift_text;
    int preserve;
    HWND device = nullptr, frames = nullptr, pitch = nullptr, formant = nullptr, shift = nullptr;
    HWND start_button = nullptr, stop_button = nullptr, inspect_button = nullptr;
    HWND spectrum_button = nullptr;
    HWND status = nullptr;
    HWND spectrum_window = nullptr;
    HFONT font = nullptr;
    bool closing = false;
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

static HWND control(HWND parent, Gui &gui, const wchar_t *type, const wchar_t *text,
                    DWORD style, int x, int y, int width, int height, int id = 0) {
    HWND child = CreateWindowExW(0, type, text, WS_CHILD | WS_VISIBLE | style,
        x, y, width, height, parent, reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)),
        GetModuleHandleW(nullptr), nullptr);
    if (!child) gui.error = GetLastError();
    if (child) SendMessageW(child, WM_SETFONT, reinterpret_cast<WPARAM>(gui.font), TRUE);
    return child;
}

static std::wstring text(HWND control) {
    int length = GetWindowTextLengthW(control);
    std::wstring value(static_cast<size_t>(length) + 1, L'\0');
    GetWindowTextW(control, value.data(), length + 1);
    value.resize(static_cast<size_t>(length));
    return value;
}

static void refresh(HWND window, Gui &gui) {
    wchar_t status[8192] = {};
    bool busy = gui.poll(gui.context, status, 8192) != 0;
    if (gui.last_status != status) {
        gui.last_status = status;
        SetWindowTextW(gui.status, status);
    }
    for (HWND item : {gui.device, gui.frames, gui.pitch, gui.formant, gui.shift,
                      gui.start_button, gui.inspect_button}) {
        EnableWindow(item, !busy && !gui.closing);
    }
    EnableWindow(gui.stop_button, busy && !gui.closing);
    EnableWindow(gui.spectrum_button, !gui.closing);
    if (gui.closing && !busy) DestroyWindow(window);
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
    info.bmiHeader.biHeight = -SPEC_H; // top-down
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
        spec->wide_button = CreateWindowExW(0, L"BUTTON", L"Wideband",
            WS_CHILD | WS_VISIBLE | BS_AUTORADIOBUTTON | WS_GROUP | WS_TABSTOP,
            12, 12, 120, 28, window, reinterpret_cast<HMENU>(SPEC_WIDE),
            GetModuleHandleW(nullptr), nullptr);
        spec->narrow_button = CreateWindowExW(0, L"BUTTON", L"Narrowband",
            WS_CHILD | WS_VISIBLE | BS_AUTORADIOBUTTON | WS_TABSTOP,
            140, 12, 130, 28, window, reinterpret_cast<HMENU>(SPEC_NARROW),
            GetModuleHandleW(nullptr), nullptr);
        spec->freeze_button = CreateWindowExW(0, L"BUTTON", L"Freeze",
            WS_CHILD | WS_VISIBLE | BS_AUTOCHECKBOX | WS_TABSTOP,
            290, 12, 100, 28, window, reinterpret_cast<HMENU>(SPEC_FREEZE),
            GetModuleHandleW(nullptr), nullptr);
        spec->view = CreateWindowExW(0, L"STATIC", L"",
            WS_CHILD | WS_VISIBLE | SS_OWNERDRAW,
            12, 52, SPEC_W, SPEC_H, window, nullptr, GetModuleHandleW(nullptr), nullptr);
        for (HWND item : {spec->wide_button, spec->narrow_button, spec->freeze_button}) {
            if (!item) { gui.error = GetLastError(); return -1; }
            SendMessageW(item, WM_SETFONT, reinterpret_cast<WPARAM>(gui.font), TRUE);
        }
        if (!spec->view) { gui.error = GetLastError(); return -1; }
        SendMessageW(spec->wide_button, BM_SETCHECK, BST_CHECKED, 0);
        if (!gui.spectrum_open(gui.context)) return -1;
        spec->started = true;
        gui.spectrum_set_wideband(gui.context, 1);
        if (!ensure_spectrum_dib(*spec)) { gui.error = GetLastError(); return -1; }
        if (!SetTimer(window, 1, 33, nullptr)) { gui.error = GetLastError(); return -1; }
        return 0;
    }
    case WM_COMMAND:
        if (HIWORD(wp) != BN_CLICKED) break;
        if (LOWORD(wp) == SPEC_WIDE) {
            spec->wideband = true;
            gui.spectrum_set_wideband(gui.context, 1);
        } else if (LOWORD(wp) == SPEC_NARROW) {
            spec->wideband = false;
            gui.spectrum_set_wideband(gui.context, 0);
        } else if (LOWORD(wp) == SPEC_FREEZE) {
            spec->frozen = SendMessageW(spec->freeze_button, BM_GETCHECK, 0, 0) == BST_CHECKED;
            gui.spectrum_set_frozen(gui.context, spec->frozen ? 1 : 0);
        }
        return 0;
    case WM_TIMER:
        update_spectrum_image(*spec);
        return 0;
    case WM_DRAWITEM: {
        auto *item = reinterpret_cast<DRAWITEMSTRUCT *>(lp);
        if (item && item->hwndItem == spec->view && spec->dib && spec->bits) {
            HDC memory = CreateCompatibleDC(item->hDC);
            if (memory) {
                HGDIOBJ previous = SelectObject(memory, spec->dib);
                BitBlt(item->hDC, 0, 0, SPEC_W, SPEC_H, memory, 0, 0, SRCCOPY);
                SelectObject(memory, previous);
                DeleteDC(memory);
            }
            return TRUE;
        }
        break;
    }
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
    wc.hbrBackground = reinterpret_cast<HBRUSH>(COLOR_BTNFACE + 1);
    wc.lpszClassName = class_name;
    if (!RegisterClassW(&wc) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) {
        gui.error = GetLastError();
        return;
    }
    auto *spec = new SpectrumGui{};
    spec->owner = &gui;
    RECT bounds{0, 0, SPEC_W + 24, SPEC_H + 64};
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

static LRESULT CALLBACK window_proc(HWND window, UINT message, WPARAM wp, LPARAM lp) {
    auto *gui = reinterpret_cast<Gui *>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
        gui = static_cast<Gui *>(reinterpret_cast<CREATESTRUCTW *>(lp)->lpCreateParams);
        SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(gui));
    }
    if (!gui) return DefWindowProcW(window, message, wp, lp);
    switch (message) {
    case WM_CREATE: {
        gui->font = CreateFontW(-16, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
            DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY, DEFAULT_PITCH, L"Segoe UI");
        if (!gui->font) { gui->error = GetLastError(); return -1; }
        control(window, *gui, L"STATIC", L"Rust Low-Latency Voice Changer", 0, 20, 16, 560, 24);
        control(window, *gui, L"STATIC", L"Device", 0, 20, 56, 110, 24);
        gui->device = control(window, *gui, L"COMBOBOX", L"",
            CBS_DROPDOWN | CBS_AUTOHSCROLL | WS_VSCROLL | WS_TABSTOP,
            140, 52, 480, 240, 201);
        for (auto name = gui->devices; *name; name += wcslen(name) + 1)
            SendMessageW(gui->device, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(name));
        SetWindowTextW(gui->device, gui->device_text);
        SendMessageW(gui->device, CB_LIMITTEXT, 512, 0);
        control(window, *gui, L"STATIC", L"Frames", 0, 20, 96, 110, 24);
        gui->frames = control(window, *gui, L"EDIT", gui->frames_text,
            WS_BORDER | ES_AUTOHSCROLL | WS_TABSTOP, 140, 92, 100, 28, 202);
        SendMessageW(gui->frames, EM_SETLIMITTEXT, 16, 0);
        control(window, *gui, L"STATIC", L"48,000 Hz | Input 1 -> output 1", 0, 260, 96, 360, 24);
        control(window, *gui, L"STATIC", L"Pitch", 0, 20, 136, 110, 24);
        gui->pitch = control(window, *gui, L"EDIT", gui->pitch_text,
            WS_BORDER | ES_AUTOHSCROLL | WS_TABSTOP, 140, 132, 100, 28, 203);
        SendMessageW(gui->pitch, EM_SETLIMITTEXT, 32, 0);
        control(window, *gui, L"STATIC", L"-12 to +12 semitones", 0, 260, 136, 360, 24);
        gui->formant = control(window, *gui, L"BUTTON", L"Preserve formants",
            BS_AUTOCHECKBOX | WS_TABSTOP, 140, 172, 300, 28, 204);
        SendMessageW(gui->formant, BM_SETCHECK, gui->preserve ? BST_CHECKED : BST_UNCHECKED, 0);
        control(window, *gui, L"STATIC", L"Formant shift", 0, 20, 216, 110, 24);
        gui->shift = control(window, *gui, L"EDIT", gui->shift_text,
            WS_BORDER | ES_AUTOHSCROLL | WS_TABSTOP, 140, 212, 100, 28, 206);
        SendMessageW(gui->shift, EM_SETLIMITTEXT, 32, 0);
        control(window, *gui, L"STATIC", L"-12 to +12 semitones (0 = preserve)", 0, 260, 216, 360, 24);
        gui->inspect_button = control(window, *gui, L"BUTTON", L"Inspect",
            WS_TABSTOP, 20, 258, 120, 34, INSPECT);
        gui->start_button = control(window, *gui, L"BUTTON", L"Start",
            WS_TABSTOP, 154, 258, 120, 34, START);
        gui->stop_button = control(window, *gui, L"BUTTON", L"Stop",
            WS_TABSTOP, 288, 258, 120, 34, STOP);
        gui->spectrum_button = control(window, *gui, L"BUTTON", L"Spectrum",
            WS_TABSTOP, 422, 258, 120, 34, SPECTRUM);
        control(window, *gui, L"STATIC", L"Stop before changing settings. Inspect opens the driver without audio buffers.",
            0, 20, 308, 600, 40);
        gui->status = control(window, *gui, L"EDIT", L"",
            WS_BORDER | ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL | WS_VSCROLL | WS_TABSTOP,
            20, 356, 600, 270, 205);
        if (gui->error) return -1;
        if (!SetTimer(window, 1, 100, nullptr)) { gui->error = GetLastError(); return -1; }
        refresh(window, *gui);
        return 0;
    }
    case WM_COMMAND:
        if (HIWORD(wp) != BN_CLICKED || gui->closing) break;
        if (LOWORD(wp) == START || LOWORD(wp) == INSPECT) {
            auto device = text(gui->device), frames = text(gui->frames), pitch = text(gui->pitch);
            auto shift = text(gui->shift);
            gui->start(gui->context, device.c_str(), frames.c_str(), pitch.c_str(), shift.c_str(),
                SendMessageW(gui->formant, BM_GETCHECK, 0, 0) == BST_CHECKED,
                LOWORD(wp) == INSPECT);
            refresh(window, *gui);
        } else if (LOWORD(wp) == STOP) {
            gui->stop(gui->context);
            EnableWindow(gui->stop_button, FALSE);
        } else if (LOWORD(wp) == SPECTRUM) {
            open_spectrum_window(window, *gui);
        }
        return 0;
    case WM_TIMER:
        refresh(window, *gui);
        return 0;
    case WM_CLOSE:
        if (gui->closing) return 0;
        {
            auto device = text(gui->device), frames = text(gui->frames), pitch = text(gui->pitch);
            auto shift = text(gui->shift);
            if (!gui->save(gui->context, device.c_str(), frames.c_str(), pitch.c_str(), shift.c_str(),
                SendMessageW(gui->formant, BM_GETCHECK, 0, 0) == BST_CHECKED)) return 0;
        }
        if (gui->spectrum_window) DestroyWindow(gui->spectrum_window);
        gui->closing = true;
        gui->stop(gui->context);
        SetWindowTextW(window, L"Rust Low-Latency Voice Changer - stopping...");
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
    Gui gui{context, start, stop, poll, save, spectrum_open, spectrum_close,
        spectrum_set_wideband, spectrum_set_frozen, spectrum_poll,
        device, devices, frames, pitch, shift, preserve};
    HINSTANCE instance = GetModuleHandleW(nullptr);
    const wchar_t *class_name = L"RustLowLatencyVoiceChangerWindow";
    WNDCLASSW wc{};
    wc.lpfnWndProc = window_proc;
    wc.hInstance = instance;
    wc.hCursor = LoadCursorW(nullptr, MAKEINTRESOURCEW(32512));
    wc.hbrBackground = reinterpret_cast<HBRUSH>(COLOR_BTNFACE + 1);
    wc.lpszClassName = class_name;
    if (!RegisterClassW(&wc)) return GetLastError();
    // DPI virtualization keeps native controls readable at the user's display scale.
    RECT bounds{0, 0, 640, 650};
    DWORD style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
    AdjustWindowRect(&bounds, style, FALSE);
    HWND window = CreateWindowExW(0, class_name, L"Rust Low-Latency Voice Changer", style,
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
                || IsDialogMessageW(window, &msg);
            if (!handled) {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if (result == -1) error = GetLastError();
        if (IsWindow(gui.spectrum_window)) DestroyWindow(gui.spectrum_window);
        if (IsWindow(window)) DestroyWindow(window);
    }
    if (gui.font) DeleteObject(gui.font);
    UnregisterClassW(class_name, instance);
    UnregisterClassW(L"RustLowLatencyVoiceChangerSpectrum", instance);
    return error;
}
