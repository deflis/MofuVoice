// Native controls only. All driver work is delegated to the Rust audio worker.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <string>

using Start = void (*)(void *, const wchar_t *, const wchar_t *, const wchar_t *, int, int);
using Stop = void (*)(void *);
using Poll = int (*)(void *, wchar_t *, size_t);
extern "C" void voice_gui_message(const wchar_t *title, const wchar_t *message, int error) {
    MessageBoxW(nullptr, message, title, MB_OK | (error ? MB_ICONERROR : MB_ICONINFORMATION));
}
enum { START = 101, STOP, INSPECT };
struct Gui {
    void *context;
    Start start;
    Stop stop;
    Poll poll;
    const wchar_t *device_text, *devices, *frames_text, *pitch_text;
    int preserve;
    HWND device = nullptr, frames = nullptr, pitch = nullptr, formant = nullptr;
    HWND start_button = nullptr, stop_button = nullptr, inspect_button = nullptr;
    HWND status = nullptr;
    HFONT font = nullptr;
    bool closing = false;
    DWORD error = 0;
    std::wstring last_status;
};

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
    for (HWND item : {gui.device, gui.frames, gui.pitch, gui.formant,
                      gui.start_button, gui.inspect_button}) {
        EnableWindow(item, !busy && !gui.closing);
    }
    EnableWindow(gui.stop_button, busy && !gui.closing);
    if (gui.closing && !busy) DestroyWindow(window);
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
        control(window, *gui, L"STATIC", L"-12 to +12 semitones (0 = bypass)", 0, 260, 136, 360, 24);
        gui->formant = control(window, *gui, L"BUTTON", L"Preserve formants",
            BS_AUTOCHECKBOX | WS_TABSTOP, 140, 172, 300, 28, 204);
        SendMessageW(gui->formant, BM_SETCHECK, gui->preserve ? BST_CHECKED : BST_UNCHECKED, 0);
        gui->inspect_button = control(window, *gui, L"BUTTON", L"Inspect",
            WS_TABSTOP, 20, 218, 120, 34, INSPECT);
        gui->start_button = control(window, *gui, L"BUTTON", L"Start",
            WS_TABSTOP, 154, 218, 120, 34, START);
        gui->stop_button = control(window, *gui, L"BUTTON", L"Stop",
            WS_TABSTOP, 288, 218, 120, 34, STOP);
        control(window, *gui, L"STATIC", L"Stop before changing settings. Inspect opens the driver without audio buffers.",
            0, 20, 268, 600, 40);
        gui->status = control(window, *gui, L"EDIT", L"",
            WS_BORDER | ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL | WS_VSCROLL | WS_TABSTOP,
            20, 316, 600, 270, 205);
        if (gui->error) return -1;
        if (!SetTimer(window, 1, 100, nullptr)) { gui->error = GetLastError(); return -1; }
        refresh(window, *gui);
        return 0;
    }
    case WM_COMMAND:
        if (HIWORD(wp) != BN_CLICKED || gui->closing) break;
        if (LOWORD(wp) == START || LOWORD(wp) == INSPECT) {
            auto device = text(gui->device), frames = text(gui->frames), pitch = text(gui->pitch);
            gui->start(gui->context, device.c_str(), frames.c_str(), pitch.c_str(),
                SendMessageW(gui->formant, BM_GETCHECK, 0, 0) == BST_CHECKED,
                LOWORD(wp) == INSPECT);
            refresh(window, *gui);
        } else if (LOWORD(wp) == STOP) {
            gui->stop(gui->context);
            EnableWindow(gui->stop_button, FALSE);
        }
        return 0;
    case WM_TIMER:
        refresh(window, *gui);
        return 0;
    case WM_CLOSE:
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
    const wchar_t *devices, const wchar_t *frames, const wchar_t *pitch, int preserve,
    Start start, Stop stop, Poll poll) {
    Gui gui{context, start, stop, poll, device, devices, frames, pitch, preserve};
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
    RECT bounds{0, 0, 640, 610};
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
            if (!IsDialogMessageW(window, &msg)) {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if (result == -1) error = GetLastError();
        if (IsWindow(window)) DestroyWindow(window);
    }
    if (gui.font) DeleteObject(gui.font);
    UnregisterClassW(class_name, instance);
    return error;
}
