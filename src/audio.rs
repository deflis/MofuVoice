//! Audio-API-independent mono DSP. Backends own buffer layout and callback lifetime.
use crate::{
    dsp::Sample,
    gate::{GateSettings, NoiseGate},
    level::{AutoLevel, LevelSettings},
    pitch::{PitchSettings, PitchShifter},
};
use std::error::Error;

pub struct DspSettings {
    pub eq: crate::eq::EqSettings,
    pub rumble_filter: bool,
    pub deesser: bool,
    pub deesser_parameters: crate::deesser::DeEsserParameters,
    pub pitch: PitchSettings,
    pub gate: GateSettings,
    pub level: LevelSettings,
}

impl From<&crate::cli::Options> for DspSettings {
    fn from(options: &crate::cli::Options) -> Self {
        Self {
            eq: options.eq.clone(),
            rumble_filter: options.rumble_filter,
            deesser: options.deesser,
            deesser_parameters: options.deesser_parameters,
            pitch: options.pitch,
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
    pub(crate) deesser: crate::deesser::DeEsser,
}

impl DspChain {
    pub fn new(settings: DspSettings) -> Result<Self, Box<dyn Error>> {
        Self::with_sample_rate(settings, crate::RATE)
    }
    pub fn with_sample_rate(settings: DspSettings, rate: u32) -> Result<Self, Box<dyn Error>> {
        crate::validate_sample_rate(rate)?;
        Ok(Self {
            eq: settings.eq.build_with_sample_rate(rate)?,
            rumble: crate::rumble::RumbleFilter::with_sample_rate(settings.rumble_filter, rate),
            pitch: settings.pitch.build()?,
            gate: settings.gate.build_with_sample_rate(rate)?,
            level: settings.level.build_with_sample_rate(rate)?,
            deesser: crate::deesser::DeEsser::with_sample_rate(
                settings.deesser,
                settings.deesser_parameters,
                rate,
            )?,
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
        self.deesser.process_in_place(output);
        nonzero
    }
}

/// Typed, preallocated callback storage for CPAL and other native adapters.
pub struct AudioProcessor<T: Sample> {
    chain: DspChain,
    scratch: Vec<T>,
}
impl<T: Sample> AudioProcessor<T> {
    /// Configure DSP for the actual device rate; no implicit resampling.
    pub fn new(
        rate: u32,
        max_frames: usize,
        settings: DspSettings,
    ) -> Result<Self, Box<dyn Error>> {
        crate::validate_sample_rate(rate)?;
        if !(1..=8192).contains(&max_frames) {
            return Err("DSP requires a maximum block size of 1..8192 frames".into());
        }
        Ok(Self {
            chain: DspChain::with_sample_rate(settings, rate)?,
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
