use rust_low_latency_voice_changer::pitch::PitchShifter;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct CountingAllocator;
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}
// SAFETY: every allocation operation is forwarded unchanged to the system allocator;
// counters are thread-local so other concurrently running tests are not counted.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ENABLED.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCATIONS.try_with(|n| n.set(n.get() + 1));
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if ENABLED.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCATIONS.try_with(|n| n.set(n.get() + 1));
        }
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn callback_does_not_allocate_including_first_frames_and_transitions() {
    for preserve in [false, true] {
        let mut dsp = PitchShifter::new(7.0, preserve, 0.0).unwrap();
        let mut level = rust_low_latency_voice_changer::level::LevelSettings {
            enabled: true,
            ..Default::default()
        }
        .build()
        .unwrap();
        let input = std::array::from_fn::<_, 512, _>(|i| (0.1 * (i as f64 * 0.017).sin()) as f32);
        let mut output = [0.0; 512];
        ALLOCATIONS.with(|n| n.set(0));
        ENABLED.with(|enabled| enabled.set(true));
        for pitch in [7.0, -12.0, 0.0, 12.0] {
            dsp.set_semitones(pitch).unwrap();
            for _ in 0..32 {
                dsp.process_block(&input, &mut output);
                level.process_in_place(&mut output);
            }
        }
        ENABLED.with(|enabled| enabled.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    }
}
