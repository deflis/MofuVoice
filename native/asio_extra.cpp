#include "asio.h"
#include <objbase.h>
// Keep SDK-specific C++ symbol names out of the Rust API.
extern "C" long voice_asio_get_latencies(long *input, long *output) {
    return ASIOGetLatencies(input, output);
}
// The SDK initializes COM on its first loading thread only. Each GUI worker
// must initialize and balance its own apartment before loading a driver.
extern "C" long voice_asio_com_initialize() {
    return CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
}
extern "C" void voice_asio_com_uninitialize() {
    CoUninitialize();
}
