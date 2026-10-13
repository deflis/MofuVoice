//! Audio-API-independent mono DSP. Backends own buffer layout and callback lifetime.
use crate::{
    gate::{GateSettings, NoiseGate},
    level::{AutoLevel, LevelSettings},
    pitch::{PitchShifter, Sample},
};
use std::error::Error;

pub struct DspSettings {
    pub eq: crate::eq::EqSettings,
    pub rumble_filter: bool,
    pub semitones: f64,
    pub preserve_formants: bool,
    pub formant_shift: f64,
    pub gate: GateSettings,
    pub level: LevelSettings,
}

impl From<&crate::cli::Options> for DspSettings {
    fn from(options: &crate::cli::Options) -> Self {
        Self {
            eq: options.eq,
            rumble_filter: options.rumble_filter,
            semitones: options.pitch,
            preserve_formants: options.preserve_formants,
            formant_shift: options.formant_shift,
            gate: GateSettings {
                enabled: options.gate,
                threshold: options.gate_threshold,
                attack_ms: options.gate_attack_ms,
                release_ms: options.gate_release_ms,
            },
            level: options.level,
        }
    }
}

pub struct DspChain {
    eq: crate::eq::Equalizer,
    pub(crate) rumble: crate::rumble::RumbleFilter,
    pub(crate) pitch: PitchShifter,
    pub(crate) gate: NoiseGate,
    pub(crate) level: AutoLevel,
}

impl DspChain {
    pub fn new(settings: DspSettings) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            eq: settings.eq.build()?,
            rumble: crate::rumble::RumbleFilter::new(settings.rumble_filter),
            pitch: PitchShifter::new(
                settings.semitones,
                settings.preserve_formants,
                settings.formant_shift,
            )?,
            gate: settings.gate.build()?,
            level: settings.level.build()?,
        })
    }

    /// Equal input/output lengths, scratch at least that long. No allocation or I/O.
    /// Scratch uses the same sample type, preserving integer and float neutral bypass.
    /// Process one selected mono input; the backend maps/silences output channels.
    /// Returns the nonzero device input count, before any DSP attenuation or tails.
    pub fn process_block<T: Sample>(
        &mut self,
        input: &[T],
        output: &mut [T],
        scratch: &mut [T],
    ) -> u64 {
        assert_eq!(input.len(), output.len());
        assert!(scratch.len() >= input.len());
        let nonzero = if self.rumble.enabled() || self.gate.enabled() {
            let scratch = &mut scratch[..input.len()];
            let nonzero = crate::bypass_block(input, scratch);
            self.rumble.process_in_place(scratch);
            self.gate.process_in_place(scratch);
            self.pitch.process_block(scratch, output);
            nonzero
        } else {
            self.pitch.process_block(input, output)
        };
        self.eq.process_in_place(output);
        self.level.process_in_place(output);
        nonzero
    }
}

/// Typed, preallocated callback storage for CPAL and other native adapters.
pub struct AudioProcessor<T: Sample> {
    chain: DspChain,
    scratch: Vec<T>,
}
impl<T: Sample> AudioProcessor<T> {
    /// The engine currently supports only 48 kHz. Never silently reinterpret another rate.
    pub fn new(
        rate: u32,
        max_frames: usize,
        settings: DspSettings,
    ) -> Result<Self, Box<dyn Error>> {
        if rate != crate::RATE || !(1..=8192).contains(&max_frames) {
            return Err("DSP requires 48000 Hz and a maximum block size of 1..8192 frames".into());
        }
        Ok(Self {
            chain: DspChain::new(settings)?,
            scratch: vec![T::default(); max_frames],
        })
    }
    /// Variable-size blocks up to max_frames; state continues across block boundaries.
    /// On invalid lengths, silence output and leave DSP state unchanged.
    pub fn process_block(&mut self, input: &[T], output: &mut [T]) -> Result<u64, &'static str> {
        if input.len() != output.len() || input.len() > self.scratch.len() {
            output.fill(T::default());
            return Err("Invalid audio block lengths");
        }
        Ok(self.chain.process_block(input, output, &mut self.scratch))
    }
}
