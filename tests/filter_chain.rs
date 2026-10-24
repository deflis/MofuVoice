//! Contract tests for the complete fixed chain, independent stage state and bypass.
use rust_low_latency_voice_changer::{
    audio::{AudioProcessor, DspSettings, FILTER_STAGES},
    cli::Options,
    dsp::Sample,
    RATE,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct CountingAllocator;
thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}
// SAFETY: operations are forwarded unchanged to System; counters are thread local.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCATIONS.try_with(|n| n.set(n.get() + 1));
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if COUNTING.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCATIONS.try_with(|n| n.set(n.get() + 1));
        }
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn configured() -> Options {
    let mut o = Options::default();
    o.rumble_filter = true;
    o.input_eq.enabled = true;
    o.input_eq.bands[0].gain_db = -3.0;
    o.pre_conversion_eq.enabled = true;
    o.pre_conversion_eq.bands[1].gain_db = 2.0;
    o.eq.enabled = true;
    o.eq.bands[2].gain_db = -2.0;
    o.limiter.enabled = true;
    o.limiter.ceiling_db = -12.0;
    o.gate = true;
    o.dereverb.enabled = true;
    o.deesser = true;
    o.compressor.enabled = true;
    o.level.enabled = true;
    o.level.compressor_enabled = true;
    o.reverb.enabled = true;
    o.pitch.semitones = 3.0;
    o
}

#[test]
fn displayed_stages_are_the_requested_fixed_processing_order() {
    assert_eq!(
        FILTER_STAGES.map(|s| s.id()),
        [
            "inputEq",
            "limiter",
            "noiseGate",
            "dereverb",
            "deesser",
            "compressor",
            "preConversionEq",
            "voice",
            "postConversionEq",
            "level",
            "reverb"
        ]
    );
}

#[test]
fn complete_chain_matches_independent_processors_in_all_formats() {
    fn check<T: Sample + std::fmt::Debug>() {
        let o = configured();
        let s = DspSettings::from(&o);
        let input: Vec<T> = (0..16000)
            .map(|i| {
                T::from_normalized(0.7 * (i as f64 * 0.12).sin() + 0.2 * (i as f64 * 1.1).sin())
            })
            .collect();
        let mut before = input.clone();
        rust_low_latency_voice_changer::rumble::RumbleFilter::new(s.rumble_filter)
            .process_in_place(&mut before);
        s.input_eq.build().unwrap().process_in_place(&mut before);
        s.limiter.build().unwrap().process_in_place(&mut before);
        s.gate.build().unwrap().process_in_place(&mut before);
        s.dereverb.build().unwrap().process_in_place(&mut before);
        rust_low_latency_voice_changer::deesser::DeEsser::with_parameters(
            s.deesser,
            s.deesser_parameters,
        )
        .unwrap()
        .process_in_place(&mut before);
        s.compressor.build().unwrap().process_in_place(&mut before);
        s.pre_conversion_eq
            .build()
            .unwrap()
            .process_in_place(&mut before);
        let mut expected = vec![T::default(); input.len()];
        s.pitch
            .build()
            .unwrap()
            .process_block(&before, &mut expected);
        s.eq.build().unwrap().process_in_place(&mut expected);
        s.level.build().unwrap().process_in_place(&mut expected);
        s.reverb
            .build_with_sample_rate(RATE)
            .unwrap()
            .process_in_place(&mut expected);
        for frames in [1, 63, 512] {
            let mut processor = AudioProcessor::new(RATE, frames, DspSettings::from(&o)).unwrap();
            let mut actual = vec![T::default(); input.len()];
            for (source, output) in input.chunks(frames).zip(actual.chunks_mut(frames)) {
                processor.process_block(source, output).unwrap();
            }
            assert_eq!(actual, expected, "block size {frames}");
        }
    }
    check::<f32>();
    check::<i16>();
    check::<i32>();
}

#[test]
fn voice_disabled_preserves_bits_with_non_neutral_saved_pitch() {
    let mut o = Options::default();
    o.voice_enabled = false;
    o.pitch.semitones = 7.0;
    let input = [f32::from_bits(0x7fc01234), -0.0, f32::from_bits(1), 0.25];
    let mut output = [0.0; 4];
    AudioProcessor::new(RATE, 4, DspSettings::from(&o))
        .unwrap()
        .process_block(&input, &mut output)
        .unwrap();
    assert_eq!(input.map(f32::to_bits), output.map(f32::to_bits));
    assert_eq!(o.pitch.semitones, 7.0);
}

#[test]
fn all_enabled_stages_do_not_allocate_in_callback() {
    let mut p = AudioProcessor::new(RATE, 512, DspSettings::from(&configured())).unwrap();
    let input = [0.3_f32; 512];
    let mut output = [0.0; 512];
    ALLOCATIONS.with(|n| n.set(0));
    COUNTING.with(|c| c.set(true));
    for frames in [1, 63, 512].into_iter().cycle().take(100) {
        p.process_block(&input[..frames], &mut output[..frames])
            .unwrap();
    }
    COUNTING.with(|c| c.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}
