#define WIN32_LEAN_AND_MEAN
#include <windows.h>
extern "C" void voice_gui_message(const wchar_t *title, const wchar_t *message, int error) {
    MessageBoxW(nullptr, message, title, MB_OK | (error ? MB_ICONERROR : MB_ICONINFORMATION));
}
