//! Separate process from spectrum unit tests, so the singleton analyzer cannot race.
use rust_low_latency_voice_changer::native_spectrum;
unsafe extern "C" {
    fn voice_spectrum_is_open() -> i32;
}
#[test]
fn native_window_can_focus_close_and_reopen_without_leaking_controller() {
    // SAFETY: this test owns all native calls on one thread for its whole lifetime.
    unsafe {
        assert_eq!(voice_spectrum_is_open(), 0);
        native_spectrum::open().unwrap();
        assert_eq!(voice_spectrum_is_open(), 1);
        native_spectrum::open().unwrap();
        native_spectrum::shutdown();
        assert_eq!(voice_spectrum_is_open(), 0);
        native_spectrum::open().unwrap();
        native_spectrum::shutdown();
        native_spectrum::shutdown();
        assert_eq!(voice_spectrum_is_open(), 0);
    }
}
