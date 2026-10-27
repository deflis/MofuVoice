//! A configurable chain of causal peaking biquads. No lookahead, buffering, or callback allocation.
use crate::{
    dsp::{biquad::Biquad, gain::amplitude},
    RATE,
};
use std::error::Error;

pub const FLAGS: [&str; 3] = ["--eq", "--eq-mode", "--eq-output"];
pub const KEYS: [&str; 3] = ["eq", "eq_mode", "eq_output"];
/// Upper bound for real-time biquad cost per sample in the audio callback.
pub const MAX_BANDS: usize = 16;

pub fn band_index(flag: &str) -> Option<usize> {
    let number = flag.strip_prefix("--eq-band")?;
    let index = number.parse::<usize>().ok()?.checked_sub(1)?;
    (number == (index + 1).to_string() && index < MAX_BANDS).then_some(index)
}
pub fn is_flag(flag: &str) -> bool {
    FLAGS.contains(&flag) || band_index(flag).is_some()
}
pub fn is_key(key: &str) -> bool {
    KEYS.contains(&key)
        || key
            .strip_prefix("eq_band")
            .is_some_and(|number| band_index(&format!("--eq-band{number}")).is_some())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    pub frequency: f64,
    pub gain_db: f64,
    pub q: f64,
}
impl Band {
    pub fn parse(text: &str) -> Result<Self, Box<dyn Error>> {
        let values: Vec<_> = text.split(',').collect();
        if values.len() != 3 {
            return Err("EQ band requires frequency,gain,Q".into());
        }
        let band = Self {
            frequency: values[0].parse()?,
            gain_db: values[1].parse()?,
            q: values[2].parse()?,
        };
        band.validate()?;
        Ok(band)
    }
    fn validate(self) -> Result<(), Box<dyn Error>> {
        if !self.frequency.is_finite()
            || !(40.0..=16000.0).contains(&self.frequency)
            || !self.gain_db.is_finite()
            || !(-12.0..=12.0).contains(&self.gain_db)
            || !self.q.is_finite()
            || !(0.2..=8.0).contains(&self.q)
        {
            return Err(
                "EQ: frequency 40..16000 Hz, gain -12..12 dB, Q 0.2..8 (finite values required)"
                    .into(),
            );
        }
        Ok(())
    }
    pub fn text(self) -> String {
        format!("{},{},{}", self.frequency, self.gain_db, self.q)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EqSettings {
    pub enabled: bool,
    pub bands: Vec<Band>,
    pub output_db: f64,
}
impl Default for EqSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            bands: vec![
                Band {
                    frequency: 250.0,
                    gain_db: 0.0,
                    q: 0.7,
                },
                Band {
                    frequency: 3000.0,
                    gain_db: 0.0,
                    q: 0.8,
                },
                Band {
                    frequency: 8000.0,
                    gain_db: 0.0,
                    q: 0.7,
                },
            ],
            output_db: 0.0,
        }
    }
}
impl EqSettings {
    pub fn validate(&self) -> Result<(), Box<dyn Error>> {
        if self.bands.is_empty() {
            return Err("EQ requires at least one band".into());
        }
        if self.bands.len() > MAX_BANDS {
            return Err(format!("EQ supports at most {MAX_BANDS} bands").into());
        }
        for band in &self.bands {
            band.validate()?;
        }
        if !self.output_db.is_finite() || !(-18.0..=0.0).contains(&self.output_db) {
            return Err("EQ output trim must be finite and between -18 and 0 dB".into());
        }
        Ok(())
    }
    pub fn flags(&self) -> Vec<String> {
        FLAGS
            .into_iter()
            .map(String::from)
            .chain((1..=self.bands.len()).map(|i| format!("--eq-band{i}")))
            .collect()
    }
    pub fn keys(&self) -> Vec<String> {
        KEYS.into_iter()
            .map(String::from)
            .chain((1..=self.bands.len()).map(|i| format!("eq_band{i}")))
            .collect()
    }
    pub fn arguments(&self, mode: &str) -> Vec<String> {
        [
            if self.enabled { "on" } else { "off" }.into(),
            mode.into(),
            self.output_db.to_string(),
        ]
        .into_iter()
        .chain(self.bands.iter().map(|band| band.text()))
        .collect()
    }
    /// Reserve the sum of positive band gains; this is not a true-peak limiter.
    pub fn headroom_db(&self) -> f64 {
        self.bands.iter().map(|band| band.gain_db.max(0.0)).sum()
    }
    pub fn build(&self) -> Result<Equalizer, Box<dyn Error>> {
        self.build_with_sample_rate(RATE)
    }
    pub fn build_with_sample_rate(&self, rate: u32) -> Result<Equalizer, Box<dyn Error>> {
        crate::validate_sample_rate(rate)?;
        self.validate()?;
        Ok(Equalizer {
            active: self.enabled
                && (self.output_db != 0.0 || self.bands.iter().any(|b| b.gain_db != 0.0)),
            filters: self
                .bands
                .iter()
                .filter(|band| band.gain_db != 0.0)
                .map(|band| PeakingFilter::new(*band, rate))
                .collect(),
            gain: amplitude(self.output_db - self.headroom_db()),
        })
    }
}
struct PeakingFilter {
    filter: Biquad,
}
impl PeakingFilter {
    fn new(band: Band, rate: u32) -> Self {
        // Peaking EQ coefficients: https://www.w3.org/TR/audio-eq-cookbook/
        let a = 10.0_f64.powf(band.gain_db / 40.0);
        let omega = std::f64::consts::TAU * band.frequency / rate as f64;
        let alpha = omega.sin() / (2.0 * band.q);
        let norm = 1.0 / (1.0 + alpha / a);
        Self {
            filter: Biquad::new(
                [
                    (1.0 + alpha * a) * norm,
                    -2.0 * omega.cos() * norm,
                    (1.0 - alpha * a) * norm,
                ],
                [-2.0 * omega.cos() * norm, (1.0 - alpha / a) * norm],
            ),
        }
    }
    fn process(&mut self, input: f64) -> f64 {
        self.filter.process(input)
    }
}
pub struct Equalizer {
    active: bool,
    filters: Vec<PeakingFilter>,
    gain: f64,
}
impl Equalizer {
    pub fn process_in_place(&mut self, samples: &mut [f64]) {
        if !self.active {
            return;
        } // Neutral settings preserve every input bit.
        for sample in samples {
            let mut value = *sample;
            if !value.is_finite() {
                for filter in &mut self.filters {
                    filter.filter.reset();
                }
                *sample = 0.0;
                continue;
            }
            // Attenuate before filtering to leave internal headroom as well.
            value = value.clamp(-16.0, 16.0) * self.gain;
            for filter in &mut self.filters {
                value = filter.process(value);
            }
            *sample = value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn variable_band_counts_preserve_neutral_bits_and_validate_every_band() {
        for count in [1, 4, 8] {
            let mut settings = EqSettings {
                enabled: true,
                bands: vec![
                    Band {
                        frequency: 1000.0,
                        gain_db: 0.0,
                        q: 1.0
                    };
                    count
                ],
                output_db: 0.0,
            };
            let input = [f64::from_bits(0x7ff8_0000_0000_1234), -0.0, 0.4];
            let mut output = input;
            settings.build().unwrap().process_in_place(&mut output);
            assert_eq!(output.map(f64::to_bits), input.map(f64::to_bits));
            settings.bands[count - 1].q = f64::NAN;
            assert!(settings.build().is_err());
        }
        assert!(EqSettings {
            bands: Vec::new(),
            ..Default::default()
        }
        .build()
        .is_err());
        assert!(EqSettings {
            bands: vec![
                Band {
                    frequency: 1000.0,
                    gain_db: 0.0,
                    q: 1.0
                };
                MAX_BANDS + 1
            ],
            ..Default::default()
        }
        .build()
        .is_err());
        assert_eq!(
            band_index(&format!("--eq-band{MAX_BANDS}")),
            Some(MAX_BANDS - 1)
        );
        assert_eq!(band_index(&format!("--eq-band{}", MAX_BANDS + 1)), None);
    }

    #[test]
    fn zero_gain_bands_are_omitted_from_the_processing_chain() {
        let mut settings = EqSettings {
            enabled: true,
            bands: vec![
                Band {
                    frequency: 250.0,
                    gain_db: 0.0,
                    q: 0.7,
                };
                MAX_BANDS
            ],
            output_db: 0.0,
        };
        settings.bands[7].gain_db = 2.0;

        let equalizer = settings.build().unwrap();

        assert_eq!(equalizer.filters.len(), 1);
    }

    #[test]
    fn neutral_and_disabled_preserve_bits() {
        let original = [
            f64::from_bits(0x7ff8_0000_0000_1234),
            -0.0,
            f64::from_bits(1),
            f64::INFINITY,
            0.4,
        ];
        for enabled in [false, true] {
            let mut settings = EqSettings {
                enabled,
                ..Default::default()
            };
            if !enabled {
                settings.bands[0].gain_db = 12.0;
            }
            let mut samples = original;
            settings.build().unwrap().process_in_place(&mut samples);
            assert_eq!(samples.map(f64::to_bits), original.map(f64::to_bits));
        }
    }
    #[test]
    fn response_matches_band_gain_and_headroom() {
        for index in 0..EqSettings::default().bands.len() {
            for gain in [-12.0, -3.0, 6.0, 12.0] {
                let mut settings = EqSettings {
                    enabled: true,
                    ..Default::default()
                };
                settings.bands[index].gain_db = gain;
                let frequency = settings.bands[index].frequency;
                let mut samples: Vec<f64> = (0..RATE)
                    .map(|i| {
                        0.1 * (std::f64::consts::TAU * frequency * i as f64 / RATE as f64).sin()
                    })
                    .collect();
                settings.build().unwrap().process_in_place(&mut samples);
                let power = samples[24000..].iter().map(|x| (*x).powi(2)).sum::<f64>() / 24000.0;
                let measured = 10.0 * (power / 0.005).log10();
                assert!(
                    (measured - (gain - settings.headroom_db())).abs() < 0.01,
                    "{index}: {measured}"
                );
            }
        }
    }
    #[test]
    fn rejects_invalid_settings_even_when_disabled() {
        for text in [
            "250,0",
            "NaN,0,1",
            "250,inf,1",
            "250,0,0",
            "24000,0,1",
            "250,13,1",
        ] {
            assert!(Band::parse(text).is_err());
        }
        for output_db in [f64::NAN, 1.0, -19.0] {
            assert!(EqSettings {
                output_db,
                ..Default::default()
            }
            .build()
            .is_err());
        }
    }
    #[test]
    fn invalid_input_resets_state_and_extreme_filters_remain_finite() {
        let settings = EqSettings {
            enabled: true,
            bands: vec![
                Band {
                    frequency: 40.0,
                    gain_db: 12.0,
                    q: 8.0,
                };
                3
            ],
            output_db: 0.0,
        };
        let mut filter = settings.build().unwrap();
        let mut samples = [0.9_f64; 8192];
        filter.process_in_place(&mut samples);
        assert!(samples.iter().all(|x| x.is_finite()));
        let mut invalid = [f64::NAN, 0.0];
        filter.process_in_place(&mut invalid);
        assert_eq!(invalid, [0.0; 2]);
    }
}
