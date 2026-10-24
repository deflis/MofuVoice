//! Causal RMS leveling, soft-knee peak compression and a zero-lookahead peak guard.
//! Envelope response times change gain, not sample positions. No audio is buffered.
use crate::{
    dsp::{
        envelope::update_rate,
        gain::{amplitude, amplitude_db, power, upper_db_boundary},
        Sample,
    },
    RATE,
};
use std::{error::Error, fmt};

// Detector state remains sample-rate, while the expensive dB targets update
// at 1/8 sample rate. Gain envelopes still smooth every sample.
const TARGET_INTERVAL: usize = 8;

pub const FLAGS: [&str; 8] = [
    "--level-target",
    "--level-max-gain",
    "--level-floor",
    "--level-response",
    "--compress-threshold",
    "--compress-ratio",
    "--compress-attack",
    "--compress-release",
];
pub const KEYS: [&str; 8] = [
    "level_target_db",
    "level_max_gain_db",
    "level_floor_db",
    "level_response_ms",
    "compress_threshold_db",
    "compress_ratio",
    "compress_attack_ms",
    "compress_release_ms",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LevelSettings {
    pub enabled: bool,
    pub compressor_enabled: bool,
    pub target_db: f64,
    pub max_gain_db: f64,
    pub floor_db: f64,
    pub response_ms: f64,
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
}
impl Default for LevelSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            compressor_enabled: false,
            target_db: -20.0,
            max_gain_db: 9.0,
            floor_db: -50.0,
            response_ms: 1500.0,
            threshold_db: -12.0,
            ratio: 3.0,
            attack_ms: 5.0,
            release_ms: 150.0,
        }
    }
}
#[derive(Debug)]
pub struct InvalidLevel(pub String);
impl fmt::Display for InvalidLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for InvalidLevel {}
impl LevelSettings {
    pub fn values(self) -> [f64; 8] {
        [
            self.target_db,
            self.max_gain_db,
            self.floor_db,
            self.response_ms,
            self.threshold_db,
            self.ratio,
            self.attack_ms,
            self.release_ms,
        ]
    }
    pub fn validate(self) -> Result<(), InvalidLevel> {
        let ranges = [
            (-36.0, -6.0),
            (0.0, 18.0),
            (-90.0, -30.0),
            (100.0, 10000.0),
            (-36.0, 0.0),
            (1.0, 20.0),
            (0.1, 100.0),
            (10.0, 2000.0),
        ];
        for ((flag, value), (min, max)) in FLAGS.iter().zip(self.values()).zip(ranges) {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(InvalidLevel(format!(
                    "{flag} must be finite and between {min} and {max}"
                )));
            }
        }
        if self.floor_db >= self.target_db {
            return Err(InvalidLevel(
                "level floor must be below target RMS level".into(),
            ));
        }
        Ok(())
    }
    pub fn build(self) -> Result<AutoLevel, InvalidLevel> {
        self.build_with_sample_rate(RATE)
    }
    pub fn build_with_sample_rate(self, sample_rate: u32) -> Result<AutoLevel, InvalidLevel> {
        crate::validate_sample_rate(sample_rate).map_err(|e| InvalidLevel(e.into()))?;
        self.validate()?;
        Ok(AutoLevel {
            settings: self,
            compression_start: amplitude(self.threshold_db - 3.0),
            floor_amplitude: upper_db_boundary(amplitude(self.floor_db), self.floor_db, |v| {
                amplitude_db(v, 1e-12)
            }),
            floor_power: upper_db_boundary(power(self.floor_db), self.floor_db, |v| {
                amplitude_db(v.sqrt(), 1e-12)
            }),
            power: 0.0,
            peak: 0.0,
            level_gain_db: 0.0,
            compression_db: 0.0,
            guard_gain: 1.0,
            control_phase: 0,
            cached_level_target_db: 0.0,
            cached_compression_target_db: 0.0,
            rms_rate: update_rate(30.0, sample_rate as f64),
            level_rate: update_rate(self.response_ms, sample_rate as f64),
            attack_rate: update_rate(self.attack_ms, sample_rate as f64),
            release_rate: update_rate(self.release_ms, sample_rate as f64),
            guard_release_rate: update_rate(
                if self.compressor_enabled {
                    self.release_ms
                } else {
                    150.0
                },
                sample_rate as f64,
            ),
        })
    }
}
fn reduction(level: f64, threshold: f64, ratio: f64) -> f64 {
    let over = level - threshold;
    let slope = 1.0 - 1.0 / ratio;
    if over <= -3.0 {
        0.0
    } else if over < 3.0 {
        slope * (over + 3.0).powi(2) / 12.0
    } else {
        slope * over
    }
}
pub struct AutoLevel {
    settings: LevelSettings,
    compression_start: f64,
    floor_amplitude: f64,
    floor_power: f64,
    power: f64,
    peak: f64,
    level_gain_db: f64,
    compression_db: f64,
    guard_gain: f64,
    control_phase: usize,
    cached_level_target_db: f64,
    cached_compression_target_db: f64,
    rms_rate: f64,
    level_rate: f64,
    attack_rate: f64,
    release_rate: f64,
    guard_release_rate: f64,
}
impl AutoLevel {
    fn compression_target(&self) -> f64 {
        // The 6 dB soft knee starts 3 dB below the threshold. Keep the
        // original dB curve above it, including both knee boundaries.
        if self.settings.ratio == 1.0 || self.peak < self.compression_start {
            0.0
        } else {
            reduction(
                amplitude_db(self.peak, 1e-12),
                self.settings.threshold_db,
                self.settings.ratio,
            )
        }
    }
    pub fn process_in_place<T: Sample>(&mut self, samples: &mut [T]) {
        if !self.settings.enabled && !self.settings.compressor_enabled {
            return;
        } // Preserve every bypass bit, including NaNs.
        for sample in samples {
            let update_target = self.control_phase == 0;
            let input = sample.normalized();
            // Invalid wet samples are silenced without poisoning detector state.
            let input = if input.is_finite() {
                input.clamp(-16.0, 16.0)
            } else {
                0.0
            };
            if self.settings.enabled {
                self.power += (input * input - self.power) * self.rms_rate;
                if self.power > self.floor_power && input.abs() > self.floor_amplitude {
                    if update_target {
                        let rms_db = amplitude_db(self.power.sqrt(), 1e-12);
                        self.cached_level_target_db = (self.settings.target_db - rms_db)
                            .clamp(-18.0, self.settings.max_gain_db);
                    }
                    self.level_gain_db +=
                        (self.cached_level_target_db - self.level_gain_db) * self.level_rate;
                }
            }
            let leveled = input * amplitude(self.level_gain_db);
            let compressed = if self.settings.compressor_enabled {
                self.peak += (0.0 - self.peak) * self.rms_rate;
                self.peak = self.peak.max(leveled.abs());
                if update_target {
                    self.cached_compression_target_db = self.compression_target();
                }
                let target = self.cached_compression_target_db;
                let rate = if target > self.compression_db {
                    self.attack_rate
                } else {
                    self.release_rate
                };
                self.compression_db += (target - self.compression_db) * rate;
                // A zero target must still release existing attenuation. Skip the
                // conversion only when the smoothed attenuation itself is zero.
                if self.compression_db == 0.0 {
                    leveled
                } else {
                    leveled * amplitude(-self.compression_db)
                }
            } else {
                leveled
            };
            // Immediate peak guard followed by smooth recovery; no lookahead or delayed samples.
            let ceiling_gain = (amplitude(-1.0) / compressed.abs().max(1e-12)).min(1.0);
            self.guard_gain += (1.0 - self.guard_gain) * self.guard_release_rate;
            self.guard_gain = self.guard_gain.min(ceiling_gain);
            *sample = T::from_normalized(compressed * self.guard_gain);

            self.control_phase += 1;
            if self.control_phase == TARGET_INTERVAL {
                self.control_phase = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_comparisons_match_db_predicates_at_ulp_boundaries() {
        for value in -9000..=-3000 {
            let settings = LevelSettings {
                enabled: true,
                floor_db: value as f64 / 100.0,
                ..Default::default()
            };
            let dsp = settings.build().unwrap();
            for offset in -64_i64..=64 {
                let amplitude =
                    f64::from_bits(dsp.floor_amplitude.to_bits().wrapping_add_signed(offset));
                let power = f64::from_bits(dsp.floor_power.to_bits().wrapping_add_signed(offset));
                assert_eq!(
                    amplitude > dsp.floor_amplitude,
                    amplitude_db(amplitude, 1e-12) > settings.floor_db,
                    "amplitude floor={}, offset={offset}",
                    settings.floor_db,
                );
                assert_eq!(
                    power > dsp.floor_power,
                    amplitude_db(power.sqrt(), 1e-12) > settings.floor_db,
                    "power floor={}, offset={offset}",
                    settings.floor_db,
                );
            }
        }
    }
    // Retain the former sample loop as an independent streaming reference.
    impl AutoLevel {
        fn process_reference<T: Sample>(&mut self, samples: &mut [T]) {
            let amplitude = |db: f64| 10.0_f64.powf(db / 20.0);
            if !self.settings.enabled {
                return;
            } // Preserve every bypass bit, including NaNs.
            for sample in samples {
                let input = sample.normalized();
                // Invalid wet samples are silenced without poisoning detector state.
                let input = if input.is_finite() {
                    input.clamp(-16.0, 16.0)
                } else {
                    0.0
                };
                self.power += (input * input - self.power) * self.rms_rate;
                let rms_db = amplitude_db(self.power.sqrt(), 1e-12);
                if rms_db > self.settings.floor_db
                    && amplitude_db(input.abs(), 1e-12) > self.settings.floor_db
                {
                    let target =
                        (self.settings.target_db - rms_db).clamp(-18.0, self.settings.max_gain_db);
                    self.level_gain_db += (target - self.level_gain_db) * self.level_rate;
                }
                let leveled = input * amplitude(self.level_gain_db);
                self.peak += (0.0 - self.peak) * self.rms_rate;
                self.peak = self.peak.max(leveled.abs());
                let target = reduction(
                    amplitude_db(self.peak, 1e-12),
                    self.settings.threshold_db,
                    self.settings.ratio,
                );
                let rate = if target > self.compression_db {
                    self.attack_rate
                } else {
                    self.release_rate
                };
                self.compression_db += (target - self.compression_db) * rate;
                let compressed = leveled * amplitude(-self.compression_db);
                // Immediate peak guard followed by smooth recovery; no lookahead or delayed samples.
                let ceiling_gain = (amplitude(-1.0) / compressed.abs().max(1e-12)).min(1.0);
                self.guard_gain += (1.0 - self.guard_gain) * self.release_rate;
                self.guard_gain = self.guard_gain.min(ceiling_gain);
                *sample = T::from_normalized(compressed * self.guard_gain);
            }
        }
    }

    #[test]
    fn compressor_target_matches_original_curve() {
        for threshold_db in [-36.0, -12.0, 0.0] {
            for ratio in [1.0, 1.01, 3.0, 20.0] {
                let mut dsp = LevelSettings {
                    threshold_db,
                    ratio,
                    ..active()
                }
                .build()
                .unwrap();
                let mut peaks: Vec<_> = (-1200..=250).map(|i| amplitude(i as f64 / 10.0)).collect();
                for boundary in [dsp.compression_start, amplitude(threshold_db + 3.0)] {
                    peaks.extend([
                        f64::from_bits(boundary.to_bits() - 1),
                        boundary,
                        f64::from_bits(boundary.to_bits() + 1),
                    ]);
                }
                peaks.extend([0.0, 1e-300]);
                for peak in peaks {
                    dsp.peak = peak;
                    let expected = reduction(amplitude_db(peak, 1e-12), threshold_db, ratio);
                    assert!(
                        (dsp.compression_target() - expected).abs() < 1e-25,
                        "threshold={threshold_db}, ratio={ratio}, peak={peak}"
                    );
                }
            }
        }
    }

    #[test]
    fn compressor_stream_matches_reference_through_attack_release_and_invalid_input() {
        // Target updates are intentionally decimated; detector and gain state
        // remain sample-rate, so compare against the former loop with a small
        // normalized-output tolerance instead of requiring bit identity.
        fn check<T: Sample>(input: &[T], settings: LevelSettings, tolerance: f64) {
            let mut expected = input.to_vec();
            let mut actual = input.to_vec();
            let mut reference = settings.build().unwrap();
            reference.process_reference(&mut expected);
            let mut optimized = settings.build().unwrap();
            for chunk in actual.chunks_mut(63) {
                optimized.process_in_place(chunk);
            }
            for (index, (a, b)) in actual.iter().zip(&expected).enumerate() {
                assert!(
                    (a.normalized() - b.normalized()).abs() <= tolerance,
                    "sample {index}: {} vs {}",
                    a.normalized(),
                    b.normalized()
                );
            }
            assert!((optimized.compression_db - reference.compression_db).abs() < 1e-4);
            if settings.ratio != 1.0 {
                assert!(reference.compression_db > 0.0, "must exercise release tail");
            }
        }
        let input: Vec<f64> = (0..RATE as usize * 2)
            .map(|i| {
                let amplitude = match i / 12000 {
                    0 => 0.01,
                    1..=3 => 0.9,
                    _ => 0.01,
                };
                amplitude * (std::f64::consts::TAU * 173.0 * i as f64 / RATE as f64).sin()
            })
            .collect();
        for threshold_db in [-36.0, -12.0, 0.0] {
            for ratio in [1.0, 3.0, 20.0] {
                let settings = LevelSettings {
                    threshold_db,
                    ratio,
                    ..active()
                };
                let mut float: Vec<f32> = input.iter().map(|x| *x as f32).collect();
                float[12001] = f32::NAN;
                float[12002] = f32::INFINITY;
                float[12003] = -32.0;
                check(&float, settings, 0.0025);
                check(
                    &input.iter().map(|x| *x as f32).collect::<Vec<_>>(),
                    settings,
                    0.0025,
                );
                check(
                    &input
                        .iter()
                        .map(|x| i16::from_normalized(*x))
                        .collect::<Vec<_>>(),
                    settings,
                    0.0025,
                );
                check(
                    &input
                        .iter()
                        .map(|x| i32::from_normalized(*x))
                        .collect::<Vec<_>>(),
                    settings,
                    0.0025,
                );
            }
        }
    }
    fn active() -> LevelSettings {
        LevelSettings {
            enabled: true,
            compressor_enabled: true,
            ..LevelSettings::default()
        }
    }
    #[test]
    fn level_and_compressor_run_independently_across_block_boundaries() {
        let input = tone(0.5, 3);
        for enabled in [false, true] {
            for compressor_enabled in [false, true] {
                let settings = LevelSettings {
                    enabled,
                    compressor_enabled,
                    target_db: -12.0,
                    threshold_db: -24.0,
                    ..LevelSettings::default()
                };
                for rate in [44100, 48000] {
                    let mut expected = input.clone();
                    let mut dsp = settings.build_with_sample_rate(rate).unwrap();
                    dsp.process_in_place(&mut expected);
                    assert_eq!(dsp.level_gain_db != 0.0, enabled);
                    assert_eq!(dsp.compression_db != 0.0, compressor_enabled);
                    if !enabled && !compressor_enabled {
                        assert_eq!(expected, input);
                    } else {
                        assert_ne!(expected, input);
                    }
                    for block in [1, 63, 512, 777] {
                        let mut actual = input.clone();
                        let mut chunked = settings.build_with_sample_rate(rate).unwrap();
                        for chunk in actual.chunks_mut(block) {
                            chunked.process_in_place(chunk);
                        }
                        assert_eq!(actual, expected);
                    }
                }
            }
        }
    }

    #[test]
    fn disabled_stage_parameters_do_not_affect_output() {
        for enabled in [false, true] {
            let settings = LevelSettings {
                enabled,
                compressor_enabled: !enabled,
                ..LevelSettings::default()
            };
            let mut changed = settings;
            if enabled {
                changed.threshold_db = -36.0;
                changed.ratio = 20.0;
                changed.attack_ms = 0.1;
                changed.release_ms = 2000.0;
            } else {
                changed.target_db = -6.0;
                changed.max_gain_db = 18.0;
                changed.response_ms = 100.0;
            }
            let mut expected = tone(0.5, 2);
            let mut actual = expected.clone();
            settings.build().unwrap().process_in_place(&mut expected);
            changed.build().unwrap().process_in_place(&mut actual);
            assert_eq!(actual, expected);
        }
    }
    fn tone(amplitude: f32, seconds: usize) -> Vec<f32> {
        (0..RATE as usize * seconds)
            .map(|i| amplitude * (std::f32::consts::TAU * 200.0 * i as f32 / RATE as f32).sin())
            .collect()
    }
    fn rms(samples: &[f32]) -> f64 {
        (samples.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
    }
    #[test]
    fn invalid_settings_fail_even_when_disabled() {
        for value in [f64::NAN, f64::INFINITY, -100000.0, 100000.0] {
            for settings in [
                LevelSettings {
                    target_db: value,
                    ..LevelSettings::default()
                },
                LevelSettings {
                    max_gain_db: value,
                    ..LevelSettings::default()
                },
                LevelSettings {
                    floor_db: value,
                    ..LevelSettings::default()
                },
                LevelSettings {
                    response_ms: value,
                    ..LevelSettings::default()
                },
                LevelSettings {
                    threshold_db: value,
                    ..LevelSettings::default()
                },
                LevelSettings {
                    ratio: value,
                    ..LevelSettings::default()
                },
                LevelSettings {
                    attack_ms: value,
                    ..LevelSettings::default()
                },
                LevelSettings {
                    release_ms: value,
                    ..LevelSettings::default()
                },
            ] {
                assert!(settings.build().is_err(), "{settings:?}");
            }
        }
        assert!(LevelSettings {
            target_db: -36.0,
            floor_db: -30.0,
            ..active()
        }
        .build()
        .is_err());
    }
    #[test]
    fn bypass_preserves_all_formats() {
        let mut dsp = LevelSettings::default().build().unwrap();
        let original = [
            f32::from_bits(0x7fc01234),
            -0.0,
            f32::from_bits(1),
            -1.0,
            f32::INFINITY,
        ];
        let mut samples = original;
        dsp.process_in_place(&mut samples);
        assert_eq!(original.map(f32::to_bits), samples.map(f32::to_bits));
        let mut int = [i32::MIN, -1, 0, 1, i32::MAX];
        let original = int;
        dsp.process_in_place(&mut int);
        assert_eq!(int, original);
        let mut short = [i16::MIN, -1, 0, 1, i16::MAX];
        let original = short;
        dsp.process_in_place(&mut short);
        assert_eq!(short, original);
    }
    #[test]
    fn levels_quiet_speech_without_raising_silence_or_low_noise() {
        let mut dsp = active().build().unwrap();
        let mut quiet = tone(0.05, 12);
        dsp.process_in_place(&mut quiet);
        let last = &quiet[quiet.len() - RATE as usize..];
        assert!(
            (amplitude_db(rms(last), 1e-12) + 20.0).abs() < 1.5,
            "{}",
            amplitude_db(rms(last), 1e-12)
        );
        let gain = dsp.level_gain_db;
        let mut silence = vec![0.0_f32; RATE as usize * 5];
        dsp.process_in_place(&mut silence);
        assert!(silence.iter().all(|x| *x == 0.0));
        assert_eq!(gain, dsp.level_gain_db);
        let mut noise = tone(0.0001, 5);
        dsp.process_in_place(&mut noise);
        assert_eq!(gain, dsp.level_gain_db);
        let mut fresh = active().build().unwrap();
        let mut noise = tone(0.0001, 5);
        let original = noise.clone();
        fresh.process_in_place(&mut noise);
        assert_eq!(noise, original);
    }
    #[test]
    fn compresses_loud_speech_and_recovers_after_shout() {
        let mut dsp = active().build().unwrap();
        let mut loud = tone(0.9, 2);
        let original = rms(&loud[48000..]);
        dsp.process_in_place(&mut loud);
        assert!(rms(&loud[48000..]) < original * 0.65);
        let mut silence = vec![0.0_f32; 48000];
        dsp.process_in_place(&mut silence);
        assert!(dsp.compression_db < 0.1);
        let mut speech = tone(0.1, 2);
        dsp.process_in_place(&mut speech);
        assert!(rms(&speech[48000..]) > 0.04);
    }

    #[test]
    fn decimated_targets_keep_loud_and_attack_release_error_bounded() {
        let settings = active();
        let window = (RATE as usize) / 100;
        for name in ["loud", "attack-release"] {
            let input: Vec<f32> = (0..RATE as usize * 4)
                .map(|i| {
                    let envelope = if name == "attack-release" && i / 12000 % 2 == 1 {
                        0.01
                    } else {
                        1.0
                    };
                    (0.9 * envelope
                        * (std::f64::consts::TAU * 173.0 * i as f64 / RATE as f64).sin())
                        as f32
                })
                .collect();
            let mut expected = input.clone();
            settings.build().unwrap().process_reference(&mut expected);
            let mut actual = input;
            settings.build().unwrap().process_in_place(&mut actual);

            let max_error = actual
                .iter()
                .zip(&expected)
                .map(|(a, b)| (a - b).abs() as f64)
                .fold(0.0_f64, f64::max);
            assert!(max_error <= 0.01, "{name} max sample error {max_error}");

            for (index, (a, b)) in actual
                .chunks_exact(window)
                .zip(expected.chunks_exact(window))
                .enumerate()
            {
                let a_rms = rms(a);
                let b_rms = rms(b);
                let level_error_db =
                    (amplitude_db(a_rms, 1e-12) - amplitude_db(b_rms, 1e-12)).abs();
                assert!(
                    level_error_db <= 0.1,
                    "{name} 10 ms window {index} level error {level_error_db} dB"
                );
            }
        }
    }

    #[test]
    fn control_targets_update_every_eight_samples() {
        let settings = LevelSettings {
            floor_db: -80.0,
            ..active()
        };
        let mut dsp = settings.build().unwrap();

        let mut first = [0.1_f32];
        dsp.process_in_place(&mut first);
        assert_eq!(dsp.control_phase, 1);
        let level_target = dsp.cached_level_target_db;
        let compression_target = dsp.cached_compression_target_db;
        let first_power = dsp.power;
        let first_peak = dsp.peak;

        let mut middle = [0.9_f32; TARGET_INTERVAL - 1];
        dsp.process_in_place(&mut middle);
        assert_eq!(dsp.control_phase, 0);
        assert_eq!(dsp.cached_level_target_db, level_target);
        assert_eq!(dsp.cached_compression_target_db, compression_target);
        assert_ne!(
            dsp.power, first_power,
            "RMS detector must update per sample"
        );
        assert_ne!(dsp.peak, first_peak, "peak detector must update per sample");

        let mut next = [0.9_f32];
        dsp.process_in_place(&mut next);
        assert_eq!(dsp.control_phase, 1);
        assert_ne!(dsp.cached_level_target_db, level_target);
        assert_ne!(dsp.cached_compression_target_db, compression_target);
    }

    #[test]
    fn chunks_are_identical_and_impulse_has_no_delay() {
        let mut whole = tone(0.15, 2);
        whole[20000] = 8.0;
        whole[20001] = f32::NAN;
        let mut chunks = whole.clone();
        let mut whole_dsp = active().build().unwrap();
        whole_dsp.process_in_place(&mut whole);
        let mut dsp = active().build().unwrap();
        let mut position = 0;
        for length in [1, 63, 128, 511, 3, 8192].into_iter().cycle() {
            if position == chunks.len() {
                break;
            }
            let end = (position + length).min(chunks.len());
            dsp.process_in_place(&mut chunks[position..end]);
            position = end;
        }
        assert_eq!(whole, chunks);
        assert_eq!(dsp.control_phase, whole_dsp.control_phase);
        assert_eq!(dsp.cached_level_target_db, whole_dsp.cached_level_target_db);
        assert_eq!(
            dsp.cached_compression_target_db,
            whole_dsp.cached_compression_target_db
        );
        assert_eq!(dsp.power, whole_dsp.power);
        assert_eq!(dsp.peak, whole_dsp.peak);
        assert_eq!(dsp.level_gain_db, whole_dsp.level_gain_db);
        assert_eq!(dsp.compression_db, whole_dsp.compression_db);
        assert_eq!(dsp.guard_gain, whole_dsp.guard_gain);
        assert!(whole.iter().all(|x| x.is_finite() && x.abs() <= 0.892));
        let mut impulse = [0.0, 4.0_f32, 0.0];
        active().build().unwrap().process_in_place(&mut impulse);
        assert_eq!(impulse[0], 0.0);
        assert!(impulse[1] > 0.0);
        assert_eq!(impulse[2], 0.0);
    }
    #[test]
    fn active_integer_formats_match_float_with_quantization() {
        let input = [0.0, 0.5, -0.8, 0.1_f32];
        let mut float = input;
        let mut int = input.map(|x| i32::from_normalized(x as f64));
        let mut short = input.map(|x| i16::from_normalized(x as f64));
        active().build().unwrap().process_in_place(&mut float);
        active().build().unwrap().process_in_place(&mut int);
        active().build().unwrap().process_in_place(&mut short);
        for i in 0..4 {
            assert!((int[i].normalized() - float[i] as f64).abs() < 1e-6);
            assert!((short[i].normalized() - float[i] as f64).abs() < 1e-4);
        }
    }
    #[test]
    fn custom_parameters_and_soft_knee() {
        let settings = LevelSettings {
            max_gain_db: 0.0,
            ratio: 1.0,
            threshold_db: 0.0,
            ..active()
        };
        let mut input = tone(0.04, 3);
        let original = input.clone();
        settings.build().unwrap().process_in_place(&mut input);
        assert_eq!(input, original);
        for boundary in [-3.0, 3.0] {
            let left = reduction(boundary - 1e-6, 0.0, 3.0);
            let right = reduction(boundary + 1e-6, 0.0, 3.0);
            assert!((left - right).abs() < 2e-6);
        }
    }
    #[test]
    fn steady_compression_does_not_add_strong_harmonics() {
        let mut samples = tone(0.9, 5);
        active().build().unwrap().process_in_place(&mut samples);
        let last = &samples[samples.len() - RATE as usize..];
        let energy = |harmonic: usize| {
            let mut real = 0.0;
            let mut imag = 0.0;
            for (i, sample) in last.iter().enumerate() {
                let phase =
                    std::f64::consts::TAU * 200.0 * harmonic as f64 * i as f64 / RATE as f64;
                real += *sample as f64 * phase.cos();
                imag += *sample as f64 * phase.sin();
            }
            real * real + imag * imag
        };
        let distortion = ((2..=7).map(energy).sum::<f64>() / energy(1)).sqrt();
        assert!(distortion < 0.02, "harmonic distortion {distortion:.5}");
    }
}
