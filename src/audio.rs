//! Audio-API-independent mono DSP. Backends own buffer layout and callback lifetime.
use crate::{
    dsp::Sample,
    gate::{GateSettings, NoiseGate},
    level::{AutoLevel, LevelSettings},
    pitch::{PitchSettings, PitchShifter},
};
use std::error::Error;

pub struct DspSettings {
    pub input_eq: crate::eq::EqSettings,
    pub pre_conversion_eq: crate::eq::EqSettings,
    pub voice_enabled: bool,
    pub compressor: crate::compressor::CompressorSettings,
    pub limiter: crate::limiter::LimiterSettings,
    pub dereverb: crate::dereverb::DereverbSettings,
    pub reverb: crate::reverb::ReverbSettings,
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
            input_eq: options.input_eq.clone(),
            pre_conversion_eq: options.pre_conversion_eq.clone(),
            voice_enabled: options.voice_enabled,
            compressor: options.compressor,
            limiter: options.limiter,
            dereverb: options.dereverb,
            reverb: options.reverb,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterStage {
    InputEq,
    Limiter,
    NoiseGate,
    Dereverb,
    Deesser,
    Compressor,
    PreConversionEq,
    Voice,
    PostConversionEq,
    Level,
    Reverb,
}
impl FilterStage {
    pub fn id(self) -> &'static str {
        match self {
            Self::Compressor => "compressor",
            Self::InputEq => "inputEq",
            Self::Limiter => "limiter",
            Self::NoiseGate => "noiseGate",
            Self::Dereverb => "dereverb",
            Self::PreConversionEq => "preConversionEq",
            Self::Deesser => "deesser",
            Self::Voice => "voice",
            Self::PostConversionEq => "postConversionEq",
            Self::Level => "level",
            Self::Reverb => "reverb",
        }
    }
}
/// The execution loop and GUI both consume this read-only definition. Level includes
/// automatic gain, compression and the legacy shared peak guard in that order.
pub const FILTER_STAGES: [FilterStage; 11] = [
    FilterStage::InputEq,
    FilterStage::Limiter,
    FilterStage::NoiseGate,
    FilterStage::Dereverb,
    FilterStage::Deesser,
    FilterStage::Compressor,
    FilterStage::PreConversionEq,
    FilterStage::Voice,
    FilterStage::PostConversionEq,
    FilterStage::Level,
    FilterStage::Reverb,
];

pub struct DspChain {
    input_eq: crate::eq::Equalizer,
    pre_conversion_eq: crate::eq::Equalizer,
    voice_enabled: bool,
    compressor: crate::compressor::Compressor,
    limiter: crate::limiter::Limiter,
    dereverb: crate::dereverb::Dereverb,
    reverb: crate::reverb::Reverb,
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
            input_eq: settings.input_eq.build_with_sample_rate(rate)?,
            pre_conversion_eq: settings.pre_conversion_eq.build_with_sample_rate(rate)?,
            voice_enabled: settings.voice_enabled,
            compressor: settings.compressor.build_with_sample_rate(rate)?,
            limiter: settings.limiter.build_with_sample_rate(rate)?,
            dereverb: settings.dereverb.build_with_sample_rate(rate)?,
            reverb: settings.reverb.build_with_sample_rate(rate)?,
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
        let scratch = &mut scratch[..input.len()];
        let nonzero = crate::bypass_block(input, scratch);
        let mut converted = false;
        for stage in FILTER_STAGES {
            if stage == FilterStage::Voice {
                if self.voice_enabled {
                    self.pitch.process_block(scratch, output);
                } else {
                    output.copy_from_slice(scratch);
                }
                converted = true;
                continue;
            }
            let samples = if converted {
                &mut *output
            } else {
                &mut *scratch
            };
            match stage {
                FilterStage::Compressor => self.compressor.process_in_place(samples),
                FilterStage::InputEq => {
                    self.rumble.process_in_place(samples);
                    self.input_eq.process_in_place(samples);
                }
                FilterStage::Limiter => self.limiter.process_in_place(samples),
                FilterStage::NoiseGate => self.gate.process_in_place(samples),
                FilterStage::Dereverb => self.dereverb.process_in_place(samples),
                FilterStage::PreConversionEq => self.pre_conversion_eq.process_in_place(samples),
                FilterStage::Deesser => self.deesser.process_in_place(samples),
                FilterStage::PostConversionEq => self.eq.process_in_place(samples),
                FilterStage::Level => self.level.process_in_place(samples),
                FilterStage::Reverb => self.reverb.process_in_place(samples),
                FilterStage::Voice => unreachable!(),
            }
        }
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
