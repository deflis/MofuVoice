//! Causal split-band de-esser for the 48 kHz mono voice pipeline.
//! First-order complementary bands sum to the original signal at unity gain.
//! No lookahead, allocation, or buffered sample delay. Not phoneme recognition.
use crate::{pitch::Sample, RATE};

pub const FLAGS: [&str; 5] = [
    "--deesser-frequency",
    "--deesser-threshold",
    "--deesser-range",
    "--deesser-attack",
    "--deesser-release",
];
pub const KEYS: [&str; 5] = [
    "deesser_frequency",
    "deesser_threshold",
    "deesser_range",
    "deesser_attack",
    "deesser_release",
];
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeEsserParameters {
    pub frequency_hz: f64,
    pub threshold_db: f64,
    pub range_db: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
}
impl Default for DeEsserParameters {
    fn default() -> Self {
        Self {
            frequency_hz: 5000.0,
            threshold_db: -24.0,
            range_db: 6.0,
            attack_ms: 0.5,
            release_ms: 60.0,
        }
    }
}
impl DeEsserParameters {
    pub fn values(self) -> [f64; 5] {
        [
            self.frequency_hz,
            self.threshold_db,
            self.range_db,
            self.attack_ms,
            self.release_ms,
        ]
    }
    pub fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.frequency_hz = value,
            1 => self.threshold_db = value,
            2 => self.range_db = value,
            3 => self.attack_ms = value,
            4 => self.release_ms = value,
            _ => unreachable!(),
        }
    }
    pub fn validate(self) -> Result<(), &'static str> {
        let limits = [
            (2000.0, 12000.0),
            (-60.0, 0.0),
            (0.0, 12.0),
            (0.1, 20.0),
            (10.0, 500.0),
        ];
        if self
            .values()
            .iter()
            .zip(limits)
            .any(|(v, (min, max))| !v.is_finite() || !(min..=max).contains(v))
        {
            return Err("De-esser requires frequency 2000..12000 Hz, threshold -60..0 dBFS, range 0..12 dB, attack 0.1..20 ms and release 10..500 ms");
        }
        Ok(())
    }
}

pub struct DeEsser {
    enabled: bool,
    parameters: DeEsserParameters,
    coefficient: f64,
    previous_input: f64,
    low: f64,
    high_power: f64,
    full_power: f64,
    detector: f64,
    attack: f64,
    release: f64,
    gain: f64,
}
impl DeEsser {
    pub fn new(enabled: bool) -> Self {
        Self::with_parameters(enabled, DeEsserParameters::default()).expect("valid defaults")
    }
    pub fn with_parameters(
        enabled: bool,
        parameters: DeEsserParameters,
    ) -> Result<Self, &'static str> {
        parameters.validate()?;
        let k = (std::f64::consts::PI * parameters.frequency_hz / RATE as f64).tan();
        Ok(Self {
            enabled,
            parameters,
            coefficient: k / (1.0 + k),
            previous_input: 0.0,
            low: 0.0,
            high_power: 0.0,
            full_power: 0.0,
            detector: (-1.0 / (0.002 * RATE as f64)).exp(),
            attack: (-1.0 / (parameters.attack_ms * 0.001 * RATE as f64)).exp(),
            release: (-1.0 / (parameters.release_ms * 0.001 * RATE as f64)).exp(),
            gain: 1.0,
        })
    }
    pub fn process_in_place<T: Sample>(&mut self, samples: &mut [T]) {
        if !self.enabled {
            return;
        }
        for sample in samples {
            let input = sample.normalized();
            if !input.is_finite() {
                self.previous_input = 0.0;
                self.low = 0.0;
                self.high_power = 0.0;
                self.full_power = 0.0;
                self.gain = 1.0;
                *sample = T::default();
                continue;
            }
            let input = input.clamp(-16.0, 16.0);
            self.low = self.coefficient * (input + self.previous_input)
                + (1.0 - 2.0 * self.coefficient) * self.low;
            self.previous_input = input;
            let high = input - self.low;
            self.high_power = self.detector * self.high_power + (1.0 - self.detector) * high * high;
            self.full_power =
                self.detector * self.full_power + (1.0 - self.detector) * input * input;
            // Smooth spectral weighting reduces vowel triggering without a binary gate.
            let fraction = self.high_power / self.full_power.max(1e-30);
            let weight = ((fraction - 0.25) / 0.25).clamp(0.0, 1.0);
            let excess =
                (10.0 * self.high_power.max(1e-30).log10() - self.parameters.threshold_db).max(0.0);
            let reduction = (excess * 0.75).min(self.parameters.range_db) * weight;
            let target = 10.0_f64.powf(-reduction / 20.0);
            let response = if target < self.gain {
                self.attack
            } else {
                self.release
            };
            self.gain = response * self.gain + (1.0 - response) * target;
            if 1.0 - self.gain < 1e-12 {
                self.gain = 1.0;
            }
            // Preserve untouched sample bits; do not quantize unity-gain integers.
            if self.gain != 1.0 {
                *sample = T::from_normalized(input + (self.gain - 1.0) * high);
            }
            for state in [&mut self.low, &mut self.high_power, &mut self.full_power] {
                if state.abs() < 1e-30 {
                    *state = 0.0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tone(hz: f64, amplitude: f64) -> Vec<f32> {
        (0..RATE)
            .map(|i| {
                (amplitude * (std::f64::consts::TAU * hz * i as f64 / RATE as f64).sin()) as f32
            })
            .collect()
    }
    fn power(x: &[f32]) -> f64 {
        x.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / x.len() as f64
    }
    #[test]
    fn strong_highs_reduce_but_vowels_and_quiet_highs_remain() {
        for (hz, amp, attenuate) in [
            (8000.0, 0.4, true),
            (250.0, 0.4, false),
            (2000.0, 0.4, false),
            (8000.0, 0.01, false),
        ] {
            let original = tone(hz, amp);
            let mut output = original.clone();
            DeEsser::new(true).process_in_place(&mut output);
            let db = 10.0 * (power(&output[24000..]) / power(&original[24000..])).log10();
            if attenuate {
                assert!(db < -3.0 && db > -6.01, "{db}");
            } else {
                assert_eq!(output, original, "{hz}");
            }
        }
    }
    #[test]
    fn bypass_bits_formats_and_zero_buffer_delay() {
        let original = [
            f32::from_bits(0x7fc01234),
            -0.0,
            f32::INFINITY,
            f32::from_bits(1),
        ];
        let mut output = original;
        DeEsser::new(false).process_in_place(&mut output);
        assert_eq!(output.map(f32::to_bits), original.map(f32::to_bits));
        let mut shorts = [i16::MIN, -1, 0, 1, i16::MAX];
        let original = shorts;
        DeEsser::new(false).process_in_place(&mut shorts);
        assert_eq!(shorts, original);
        let mut impulse = [0.5_f32, 0.0];
        DeEsser::new(true).process_in_place(&mut impulse);
        assert!(impulse[0] > 0.49);
    }
    #[test]
    fn block_continuity_invalid_recovery_and_release() {
        let mut input = tone(8000.0, 0.4);
        input[1500] = f32::NAN;
        input[2000] = f32::INFINITY;
        let mut whole = input.clone();
        let mut chunks = input;
        DeEsser::new(true).process_in_place(&mut whole);
        let mut dsp = DeEsser::new(true);
        for chunk in chunks.chunks_mut(63) {
            dsp.process_in_place(chunk);
        }
        assert_eq!(whole, chunks);
        assert!(whole.iter().all(|x| x.is_finite()));
        dsp.process_in_place(&mut vec![0.0_f32; RATE as usize * 2]);
        assert_eq!(dsp.gain, 1.0);
        let original = tone(250.0, 0.4);
        let mut output = original.clone();
        dsp.process_in_place(&mut output);
        assert_eq!(output, original);
    }
    #[test]
    fn mixed_voice_retains_low_band_and_catches_short_sibilant_burst() {
        let low = tone(250.0, 0.15);
        let high = tone(8000.0, 0.4);
        let mut mixed: Vec<f32> = low.iter().zip(&high).map(|(a, b)| a + b).collect();
        DeEsser::new(true).process_in_place(&mut mixed);
        let amplitude = |samples: &[f32], hz: f64| {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, x) in samples.iter().enumerate() {
                let phase = std::f64::consts::TAU * hz * i as f64 / RATE as f64;
                re += *x as f64 * phase.cos();
                im += *x as f64 * phase.sin();
            }
            2.0 * re.hypot(im) / samples.len() as f64
        };
        assert!((amplitude(&mixed[24000..], 250.0) / 0.15 - 1.0).abs() < 0.01);
        assert!(amplitude(&mixed[24000..], 8000.0) < 0.28);
        let mut burst = high[..960].to_vec();
        DeEsser::new(true).process_in_place(&mut burst);
        assert!(power(&burst[480..]) < power(&high[480..960]) * 0.5);
    }
    #[test]
    fn intensity_is_monotonic_and_zero_preserves_bits() {
        let input = tone(8000.0, 0.4);
        let mut previous = power(&input[24000..]);
        for range_db in [0.0, 3.0, 6.0, 9.0, 12.0] {
            let mut output = input.clone();
            let parameters = DeEsserParameters {
                range_db,
                threshold_db: -36.0,
                ..DeEsserParameters::default()
            };
            DeEsser::with_parameters(true, parameters)
                .unwrap()
                .process_in_place(&mut output);
            let current = power(&output[24000..]);
            if range_db == 0.0 {
                assert_eq!(output, input);
            } else {
                assert!(current < previous, "{range_db}");
            }
            previous = current;
        }
    }
    #[test]
    fn custom_parameters_validate_even_when_disabled_and_remain_block_continuous() {
        for index in 0..5 {
            for value in [f64::NAN, f64::INFINITY, -1e6, 1e6] {
                let mut parameters = DeEsserParameters::default();
                parameters.set(index, value);
                assert!(DeEsser::with_parameters(false, parameters).is_err());
            }
        }
        for frequency_hz in [2000.0, 12000.0] {
            let parameters = DeEsserParameters {
                frequency_hz,
                threshold_db: -40.0,
                range_db: 12.0,
                attack_ms: 0.1,
                release_ms: 500.0,
            };
            let mut whole = tone(8000.0, 0.4);
            let mut chunks = whole.clone();
            DeEsser::with_parameters(true, parameters)
                .unwrap()
                .process_in_place(&mut whole);
            let mut dsp = DeEsser::with_parameters(true, parameters).unwrap();
            for chunk in chunks.chunks_mut(31) {
                dsp.process_in_place(chunk);
            }
            assert_eq!(whole, chunks);
            assert!(whole.iter().all(|x| x.is_finite()));
        }
    }
    #[test]
    fn active_integer_formats_match_float() {
        let input = tone(8000.0, 0.4);
        let mut float = input.clone();
        let mut short: Vec<_> = input
            .iter()
            .map(|x| i16::from_normalized(*x as f64))
            .collect();
        let mut int: Vec<_> = input
            .iter()
            .map(|x| i32::from_normalized(*x as f64))
            .collect();
        DeEsser::new(true).process_in_place(&mut float);
        DeEsser::new(true).process_in_place(&mut short);
        DeEsser::new(true).process_in_place(&mut int);
        for i in 0..input.len() {
            assert!((short[i].normalized() - float[i] as f64).abs() < 1e-4);
            assert!((int[i].normalized() - float[i] as f64).abs() < 1e-6);
        }
    }
}
