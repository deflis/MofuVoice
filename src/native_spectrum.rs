//! Native window operations must be dispatched to the desktop main thread.
use crate::spectrum::{SpectrumController, BYTES_PER_PIXEL, IMAGE_HEIGHT, IMAGE_WIDTH};
use std::ffi::c_void;

unsafe extern "C" {
    fn voice_spectrum_is_open() -> i32;
    fn voice_spectrum_focus();
    fn voice_spectrum_close();
    fn voice_spectrum_open(
        context: *mut c_void,
        close: extern "C" fn(*mut c_void),
        wide: extern "C" fn(*mut c_void, i32),
        frozen: extern "C" fn(*mut c_void, i32),
        poll: extern "C" fn(*mut c_void, *mut u8, usize, *mut u64) -> i32,
    ) -> u32;
}
extern "C" fn close(context: *mut c_void) {
    // SAFETY: the native window releases the transferred Box exactly once.
    drop(unsafe { Box::from_raw(context.cast::<SpectrumController>()) });
}
extern "C" fn wide(context: *mut c_void, enabled: i32) {
    // SAFETY: the native window owns a live controller until close.
    unsafe { &*context.cast::<SpectrumController>() }.set_wideband(enabled != 0);
}
extern "C" fn frozen(context: *mut c_void, enabled: i32) {
    // SAFETY: same contract as wide.
    unsafe { &*context.cast::<SpectrumController>() }.set_frozen(enabled != 0);
}
extern "C" fn poll(
    context: *mut c_void,
    pixels: *mut u8,
    capacity: usize,
    generation: *mut u64,
) -> i32 {
    let needed = IMAGE_WIDTH * IMAGE_HEIGHT * BYTES_PER_PIXEL;
    if capacity < needed || pixels.is_null() || generation.is_null() {
        return 0;
    }
    // SAFETY: the window supplies writable buffers and its live controller.
    unsafe {
        i32::from((&*context.cast::<SpectrumController>()).copy_image(
            std::slice::from_raw_parts_mut(pixels, needed),
            &mut *generation,
        ))
    }
}
/// # Safety
/// Call only on the desktop main thread, including all subsequent window operations.
pub unsafe fn open() -> Result<(), String> {
    // SAFETY: caller guarantees main-thread access to the native singleton.
    if unsafe { voice_spectrum_is_open() } != 0 {
        unsafe { voice_spectrum_focus() };
        return Ok(());
    }
    let controller = SpectrumController::start().map_err(|error| error.to_string())?;
    let context = Box::into_raw(Box::new(controller)).cast();
    // SAFETY: native code owns context on both success and failure.
    let error = unsafe { voice_spectrum_open(context, close, wide, frozen, poll) };
    if error != 0 {
        return Err(format!("Cannot open spectrum window: Win32 error {error}"));
    }
    Ok(())
}
/// # Safety
/// Call only on the desktop main thread.
pub unsafe fn shutdown() {
    unsafe { voice_spectrum_close() };
}
