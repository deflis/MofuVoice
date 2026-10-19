use rust_low_latency_voice_changer::pitch::{PitchAlgorithm, PitchShifter};
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
fn portable_processor_does_not_allocate_with_rumble_gate_pitch_eq_level_and_deesser_enabled() {
    use rust_low_latency_voice_changer::{
        audio::{AudioProcessor, DspSettings},
        cli::Options,
        RATE,
    };
    let options = Options::parse(
        [
            "--deesser",
            "on",
            "--rumble-filter",
            "on",
            "--pitch",
            "3",
            "--gate",
            "on",
            "--auto-level",
            "on",
            "--eq",
            "on",
            "--eq-band1",
            "250,-3,0.7",
        ]
        .map(String::from),
    )
    .unwrap();
    let mut processor = AudioProcessor::<f32>::new(RATE, 512, DspSettings::from(&options)).unwrap();
    let input = [0.1; 512];
    let mut output = [0.0; 512];
    ALLOCATIONS.with(|n| n.set(0));
    ENABLED.with(|enabled| enabled.set(true));
    for frames in [1, 64, 511, 512].into_iter().cycle().take(128) {
        processor
            .process_block(&input[..frames], &mut output[..frames])
            .unwrap();
    }
    ENABLED.with(|enabled| enabled.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}

#[test]
fn callback_does_not_allocate_including_first_frames_and_transitions() {
    for algorithm in PitchAlgorithm::ALL {
        for (preserve, shift) in [(false, 0.0), (true, 0.0), (true, 2.53)] {
            if algorithm.uses_td_psola() && !preserve {
                continue;
            }
            let mut dsp = PitchShifter::new(
                7.0,
                preserve,
                shift,
                algorithm,
                rust_low_latency_voice_changer::pitch::SpectralWindows::default(),
            )
            .unwrap();
            let mut level = rust_low_latency_voice_changer::level::LevelSettings {
                enabled: true,
                compressor_enabled: true,
                ..Default::default()
            }
            .build()
            .unwrap();
            let input = std::array::from_fn::<_, 512, _>(|i| {
                ((1..=8)
                    .map(|h| 0.015 * (i as f64 * 0.017 * h as f64).cos())
                    .sum::<f64>()) as f32
            });
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
}

#[test]
fn configured_windows_do_not_allocate_in_callbacks_or_pitch_transitions() {
    use rust_low_latency_voice_changer::pitch::SpectralWindows;
    for analysis in [1024, 2048, 4096, 8192] {
        for synthesis in [256, 512, 1024, 2048] {
            if synthesis > analysis {
                continue;
            }
            for algorithm in PitchAlgorithm::SPECTRAL {
                let mut dsp = PitchShifter::new(
                    7.0,
                    true,
                    2.53,
                    algorithm,
                    SpectralWindows {
                        analysis,
                        synthesis,
                    },
                )
                .unwrap();
                let input = [0.1_f32; 511];
                let mut output = [0.0; 511];
                ALLOCATIONS.with(|n| n.set(0));
                ENABLED.with(|enabled| enabled.set(true));
                for pitch in [-12.0, 0.0, 12.0] {
                    dsp.set_semitones(pitch).unwrap();
                    for _ in 0..32 {
                        dsp.process_block(&input, &mut output);
                    }
                }
                ENABLED.with(|enabled| enabled.set(false));
                assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            }
        }
    }
}
