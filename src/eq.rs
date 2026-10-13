//! Three causal peaking biquads. No lookahead, buffering, or callback allocation.
use crate::{pitch::Sample, RATE};
use std::error::Error;

pub const FLAGS: [&str; 6] = [
    "--eq",
    "--eq-mode",
    "--eq-band1",
    "--eq-band2",
    "--eq-band3",
    "--eq-output",
];
pub const KEYS: [&str; 6] = [
    "eq",
    "eq_mode",
    "eq_band1",
    "eq_band2",
    "eq_band3",
    "eq_output",
];

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EqSettings {
    pub enabled: bool,
    pub bands: [Band; 3],
    pub output_db: f64,
}
impl Default for EqSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            bands: [
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
    pub fn validate(self) -> Result<(), Box<dyn Error>> {
        for band in self.bands {
            band.validate()?;
        }
        if !self.output_db.is_finite() || !(-18.0..=0.0).contains(&self.output_db) {
            return Err("EQ output trim must be finite and between -18 and 0 dB".into());
        }
        Ok(())
    }
    pub fn arguments(self, mode: &str) -> [String; 6] {
        [
            if self.enabled { "on" } else { "off" }.into(),
            mode.into(),
            self.bands[0].text(),
            self.bands[1].text(),
            self.bands[2].text(),
            self.output_db.to_string(),
        ]
    }
    /// Reserve the sum of positive band gains; this is not a true-peak limiter.
    pub fn headroom_db(self) -> f64 {
        self.bands.iter().map(|band| band.gain_db.max(0.0)).sum()
    }
    pub fn build(self) -> Result<Equalizer, Box<dyn Error>> {
        self.validate()?;
        Ok(Equalizer {
            active: self.enabled
                && (self.output_db != 0.0 || self.bands.iter().any(|b| b.gain_db != 0.0)),
            filters: self.bands.map(Biquad::new),
            gain: 10.0_f64.powf((self.output_db - self.headroom_db()) / 20.0),
        })
    }
}
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
    active: bool,
}
impl Biquad {
    fn new(band: Band) -> Self {
        // Peaking EQ coefficients: https://www.w3.org/TR/audio-eq-cookbook/
        let a = 10.0_f64.powf(band.gain_db / 40.0);
        let omega = std::f64::consts::TAU * band.frequency / RATE as f64;
        let alpha = omega.sin() / (2.0 * band.q);
        let norm = 1.0 / (1.0 + alpha / a);
        Self {
            b0: (1.0 + alpha * a) * norm,
            b1: -2.0 * omega.cos() * norm,
            b2: (1.0 - alpha * a) * norm,
            a1: -2.0 * omega.cos() * norm,
            a2: (1.0 - alpha / a) * norm,
            z1: 0.0,
            z2: 0.0,
            active: band.gain_db != 0.0,
        }
    }
    fn process(&mut self, input: f64) -> f64 {
        if !self.active {
            return input;
        }
        let output = self.b0 * input + self.z1;
        self.z1 = self.b1 * input - self.a1 * output + self.z2;
        self.z2 = self.b2 * input - self.a2 * output;
        if self.z1.abs() < 1e-30 {
            self.z1 = 0.0;
        }
        if self.z2.abs() < 1e-30 {
            self.z2 = 0.0;
        }
        output
    }
}
pub struct Equalizer {
    active: bool,
    filters: [Biquad; 3],
    gain: f64,
}
impl Equalizer {
    pub fn process_in_place<T: Sample>(&mut self, samples: &mut [T]) {
        if !self.active {
            return;
        } // Neutral settings preserve every input bit.
        for sample in samples {
            let mut value = sample.normalized();
            if !value.is_finite() {
                for filter in &mut self.filters {
                    filter.z1 = 0.0;
                    filter.z2 = 0.0;
                }
                *sample = T::default();
                continue;
            }
            // Attenuate before filtering to leave internal headroom as well.
            value = value.clamp(-16.0, 16.0) * self.gain;
            for filter in &mut self.filters {
                value = filter.process(value);
            }
            *sample = T::from_normalized(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn neutral_and_disabled_preserve_bits() {
        let original = [
            f32::from_bits(0x7fc01234),
            -0.0,
            f32::from_bits(1),
            f32::INFINITY,
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
            assert_eq!(samples.map(f32::to_bits), original.map(f32::to_bits));
        }
    }
    #[test]
    fn response_matches_band_gain_and_headroom() {
        for index in 0..3 {
            for gain in [-12.0, -3.0, 6.0, 12.0] {
                let mut settings = EqSettings {
                    enabled: true,
                    ..Default::default()
                };
                settings.bands[index].gain_db = gain;
                let frequency = settings.bands[index].frequency;
                let mut samples: Vec<f32> = (0..RATE)
                    .map(|i| {
                        (0.1 * (std::f64::consts::TAU * frequency * i as f64 / RATE as f64).sin())
                            as f32
                    })
                    .collect();
                settings.build().unwrap().process_in_place(&mut samples);
                let power = samples[24000..]
                    .iter()
                    .map(|x| (*x as f64).powi(2))
                    .sum::<f64>()
                    / 24000.0;
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
            bands: [Band {
                frequency: 40.0,
                gain_db: 12.0,
                q: 8.0,
            }; 3],
            output_db: 0.0,
        };
        let mut filter = settings.build().unwrap();
        let mut samples = [0.9_f32; 8192];
        filter.process_in_place(&mut samples);
        assert!(samples.iter().all(|x| x.is_finite()));
        let mut invalid = [f32::NAN, 0.0];
        filter.process_in_place(&mut invalid);
        assert_eq!(invalid, [0.0; 2]);
    }
}
