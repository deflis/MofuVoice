//! Soft downward-expander noise gate with optional noise-floor threshold tracking.
//! Zero algorithmic delay: no lookahead. Hold time is internal and not exposed.
use crate::pitch::Sample;
use crate::RATE;
use std::{error::Error, fmt};

const RATIO: f64 = 4.0;
const FLOOR_DB: f64 = -72.0;
const HOLD_MS: f64 = 40.0;
const AUTO_MARGIN_DB: f64 = 10.0;
// Silence must not make quiet room/keyboard transients open the gate.
const AUTO_MIN_THRESHOLD_DB: f64 = -45.0;
const MIN_DB: f64 = -120.0;
const DETECT_ATTACK_MS: f64 = 0.5;
const DETECT_RELEASE_MS: f64 = 10.0;
const FLOOR_FALL_MS: f64 = 50.0;
const FLOOR_RISE_MS: f64 = 3_000.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InvalidGate;
impl fmt::Display for InvalidGate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "gate attack/release must be finite and between 0.1 and 2000 ms; threshold must be auto or a finite dB value between -90 and 0",
        )
    }
}
impl Error for InvalidGate {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GateThreshold {
    Auto,
    Db(f64),
}

impl GateThreshold {
    pub fn parse(text: &str) -> Result<Self, InvalidGate> {
        if text.eq_ignore_ascii_case("auto") {
            return Ok(Self::Auto);
        }
        let value: f64 = text.parse().map_err(|_| InvalidGate)?;
        if !value.is_finite() || !(-90.0..=0.0).contains(&value) {
            return Err(InvalidGate);
        }
        Ok(Self::Db(value))
    }
}

pub fn validate_time_ms(ms: f64) -> Result<f64, InvalidGate> {
    if !ms.is_finite() || !(0.1..=2000.0).contains(&ms) {
        return Err(InvalidGate);
    }
    Ok(ms)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GateSettings {
    pub enabled: bool,
    pub threshold: GateThreshold,
    pub attack_ms: f64,
    pub release_ms: f64,
}

impl GateSettings {
    pub fn build(self) -> Result<NoiseGate, InvalidGate> {
        NoiseGate::new(
            self.enabled,
            self.threshold,
            self.attack_ms,
            self.release_ms,
        )
    }
}

fn coef(ms: f64) -> f64 {
    (-1.0 / (ms * 0.001 * RATE as f64)).exp()
}

fn to_db(level: f64) -> f64 {
    20.0 * level.max(1e-10).log10()
}

fn from_db(db: f64) -> f64 {
    10.0_f64.powf(db / 20.0)
}

/// Downward expansion gain in dB. Attack/release smoothing softens the open/close edge.
fn expansion_gain_db(level_db: f64, threshold_db: f64) -> f64 {
    let delta = level_db - threshold_db;
    if delta >= 0.0 {
        0.0
    } else {
        ((RATIO - 1.0) * delta).max(FLOOR_DB)
    }
}

pub struct NoiseGate {
    enabled: bool,
    threshold: GateThreshold,
    attack: f64,
    release: f64,
    hold_samples: u64,
    detect_attack: f64,
    detect_release: f64,
    floor_fall: f64,
    floor_rise: f64,
    envelope: f64,
    gain: f64,
    hold_remaining: u64,
    noise_floor_db: f64,
}

impl NoiseGate {
    pub fn new(
        enabled: bool,
        threshold: GateThreshold,
        attack_ms: f64,
        release_ms: f64,
    ) -> Result<Self, InvalidGate> {
        let attack_ms = validate_time_ms(attack_ms)?;
        let release_ms = validate_time_ms(release_ms)?;
        match threshold {
            GateThreshold::Auto => {}
            GateThreshold::Db(db) if db.is_finite() && (-90.0..=0.0).contains(&db) => {}
            GateThreshold::Db(_) => return Err(InvalidGate),
        }
        Ok(Self {
            enabled,
            threshold,
            attack: 1.0 - coef(attack_ms),
            release: 1.0 - coef(release_ms),
            hold_samples: (HOLD_MS * 0.001 * RATE as f64).round() as u64,
            detect_attack: 1.0 - coef(DETECT_ATTACK_MS),
            detect_release: 1.0 - coef(DETECT_RELEASE_MS),
            floor_fall: 1.0 - coef(FLOOR_FALL_MS),
            floor_rise: 1.0 - coef(FLOOR_RISE_MS),
            envelope: 0.0,
            gain: 1.0,
            hold_remaining: 0,
            noise_floor_db: -60.0,
        })
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    fn threshold_db(&self) -> f64 {
        match self.threshold {
            GateThreshold::Db(db) => db,
            GateThreshold::Auto => {
                (self.noise_floor_db + AUTO_MARGIN_DB).clamp(AUTO_MIN_THRESHOLD_DB, -6.0)
            }
        }
    }

    fn update_noise_floor(&mut self, level_db: f64) {
        if !matches!(self.threshold, GateThreshold::Auto) {
            return;
        }
        // Adapt mainly while the signal sits near the estimated floor.
        if level_db <= self.noise_floor_db + AUTO_MARGIN_DB {
            let rate = if level_db < self.noise_floor_db {
                self.floor_fall
            } else {
                self.floor_rise
            };
            self.noise_floor_db += (level_db - self.noise_floor_db) * rate;
            self.noise_floor_db = self.noise_floor_db.clamp(MIN_DB, -6.0);
        }
    }

    fn process_sample(&mut self, input: f64) -> f64 {
        let level = if input.is_finite() { input.abs() } else { 0.0 };
        if level > self.envelope {
            self.envelope += (level - self.envelope) * self.detect_attack;
        } else {
            self.envelope += (level - self.envelope) * self.detect_release;
        }
        let level_db = to_db(self.envelope);
        self.update_noise_floor(level_db);
        let threshold_db = self.threshold_db();
        let mut target = from_db(expansion_gain_db(level_db, threshold_db));
        if target >= 0.999 {
            self.hold_remaining = self.hold_samples;
            target = 1.0;
        } else if self.hold_remaining > 0 {
            self.hold_remaining -= 1;
            target = 1.0;
        }
        if target > self.gain {
            self.gain += (target - self.gain) * self.attack;
        } else {
            self.gain += (target - self.gain) * self.release;
        }
        input * self.gain
    }

    pub fn process_in_place<T: Sample>(&mut self, samples: &mut [T]) {
        if !self.enabled {
            return;
        }
        for sample in samples {
            *sample = T::from_normalized(self.process_sample(sample.normalized()));
        }
    }

    pub fn process_block<T: Sample>(&mut self, input: &[T], output: &mut [T]) {
        assert_eq!(
            input.len(),
            output.len(),
            "gate input/output lengths must match"
        );
        if !self.enabled {
            output.copy_from_slice(input);
            return;
        }
        for (sample, out) in input.iter().zip(output.iter_mut()) {
            *out = T::from_normalized(self.process_sample(sample.normalized()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_arguments() {
        assert!(NoiseGate::new(true, GateThreshold::Db(f64::NAN), 5.0, 80.0).is_err());
        assert!(NoiseGate::new(true, GateThreshold::Db(-91.0), 5.0, 80.0).is_err());
        assert!(NoiseGate::new(true, GateThreshold::Auto, 0.0, 80.0).is_err());
        assert!(NoiseGate::new(true, GateThreshold::Auto, 5.0, 2000.1).is_err());
        assert!(GateThreshold::parse("nope").is_err());
        assert_eq!(GateThreshold::parse("auto").unwrap(), GateThreshold::Auto);
        assert_eq!(
            GateThreshold::parse("-40").unwrap(),
            GateThreshold::Db(-40.0)
        );
    }

    #[test]
    fn disabled_preserves_integer_bits() {
        let mut gate = NoiseGate::new(false, GateThreshold::Auto, 5.0, 80.0).unwrap();
        let input = [i32::MIN, -1, 0, 1, i32::MAX];
        let mut output = [0; 5];
        gate.process_block(&input, &mut output);
        assert_eq!(input, output);
    }

    #[test]
    fn silence_stays_near_zero_when_open_threshold_is_high() {
        let mut gate = NoiseGate::new(true, GateThreshold::Db(-20.0), 1.0, 5.0).unwrap();
        let input = vec![0.001_f32; 4_800];
        let mut output = vec![0.0; input.len()];
        gate.process_block(&input, &mut output);
        let tail = &output[output.len() - 1_000..];
        let rms = (tail.iter().map(|s| s * s).sum::<f32>() / tail.len() as f32).sqrt();
        assert!(rms < 0.0002, "rms={rms}");
    }

    #[test]
    fn strong_tone_passes_after_attack() {
        let mut gate = NoiseGate::new(true, GateThreshold::Db(-40.0), 1.0, 20.0).unwrap();
        let input: Vec<f32> = (0..4_800)
            .map(|i| (std::f64::consts::TAU * 440.0 * i as f64 / RATE as f64).sin() as f32 * 0.5)
            .collect();
        let mut output = vec![0.0; input.len()];
        gate.process_block(&input, &mut output);
        let start = output.len() - 1_000;
        let err: f32 = input[start..]
            .iter()
            .zip(&output[start..])
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / 1_000.0;
        assert!(err < 0.02, "mean abs err={err}");
    }

    #[test]
    fn auto_threshold_learns_quiet_floor_and_opens_for_speech() {
        let mut gate = NoiseGate::new(true, GateThreshold::Auto, 2.0, 30.0).unwrap();
        let noise: Vec<f32> = vec![0.0005; 48_000];
        let mut scratch = vec![0.0; noise.len()];
        gate.process_block(&noise, &mut scratch);
        let speech: Vec<f32> = (0..24_000)
            .map(|i| (std::f64::consts::TAU * 220.0 * i as f64 / RATE as f64).sin() as f32 * 0.25)
            .collect();
        let mut out = vec![0.0; speech.len()];
        gate.process_block(&speech, &mut out);
        let peak = out[out.len() - 2_000..]
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);
        assert!(peak > 0.1, "peak={peak}");
        assert!(gate.threshold_db() > gate.noise_floor_db);
    }

    #[test]
    fn zero_additional_latency_on_impulse_when_open() {
        let mut gate = NoiseGate::new(true, GateThreshold::Db(-60.0), 0.1, 0.1).unwrap();
        // Prime the gate open with a loud tone.
        let prime = vec![0.5_f32; 2_000];
        let mut scratch = vec![0.0; prime.len()];
        gate.process_block(&prime, &mut scratch);
        let mut input = vec![0.0_f32; 64];
        input[0] = 1.0;
        let mut output = vec![0.0; 64];
        gate.process_block(&input, &mut output);
        assert!(output[0].abs() > 0.9, "out0={}", output[0]);
    }

    #[test]
    fn auto_suppresses_repeated_quiet_clicks_after_silence() {
        let mut gate = NoiseGate::new(true, GateThreshold::Auto, 5.0, 80.0).unwrap();
        gate.process_in_place(&mut vec![0.0_f32; RATE as usize * 2]);
        let amplitude = from_db(-52.0) as f32;
        let mut clicks = vec![0.0_f32; RATE as usize];
        for start in (0..clicks.len()).step_by(RATE as usize / 10) {
            // Three millisecond alternating transients, ten keystrokes per second.
            for i in 0..144 {
                clicks[start + i] = if i % 2 == 0 { amplitude } else { -amplitude };
            }
        }
        gate.process_in_place(&mut clicks);
        let peak = clicks.iter().map(|x| x.abs()).fold(0.0_f32, f32::max);
        assert!(peak < amplitude * 0.1, "click attenuation <20 dB: {peak}");
        assert_eq!(gate.threshold_db(), AUTO_MIN_THRESHOLD_DB);
        assert_eq!(gate.hold_remaining, 0, "clicks must not trigger hold");
    }

    #[test]
    fn auto_opens_for_quiet_voice_after_silence_and_closes_after_speech() {
        let mut gate = NoiseGate::new(true, GateThreshold::Auto, 5.0, 80.0).unwrap();
        gate.process_in_place(&mut vec![0.0_f32; RATE as usize * 2]);
        let amplitude = from_db(-34.0) as f32;
        let mut voice: Vec<f32> = (0..RATE as usize / 2)
            .map(|i| {
                (std::f64::consts::TAU * 180.0 * i as f64 / RATE as f64).sin() as f32 * amplitude
            })
            .collect();
        let original = voice.clone();
        gate.process_in_place(&mut voice);
        // Gain attack must open promptly without buffering the speech onset.
        let onset = (0..960).find(|&i| {
            original[i].abs() > amplitude * 0.5 && voice[i].abs() > original[i].abs() * 0.8
        });
        assert!(onset.is_some(), "voice must open within 20 ms");
        for (input, output) in original[2400..].iter().zip(&voice[2400..]) {
            assert!((input - output).abs() < amplitude * 0.001);
        }
        gate.process_in_place(&mut vec![0.0_f32; RATE as usize]);
        let mut noise = vec![from_db(-52.0) as f32; RATE as usize / 2];
        gate.process_in_place(&mut noise);
        assert!(noise.iter().all(|x| x.abs() < from_db(-52.0) as f32 * 0.1));
    }

    #[test]
    fn manual_threshold_can_still_be_lower_than_auto_minimum() {
        let mut gate = NoiseGate::new(true, GateThreshold::Db(-80.0), 5.0, 80.0).unwrap();
        let mut quiet = vec![from_db(-65.0) as f32; RATE as usize / 2];
        gate.process_in_place(&mut quiet);
        assert_eq!(gate.threshold_db(), -80.0);
        assert!(quiet.last().unwrap().abs() > from_db(-65.0) as f32 * 0.99);
    }
}
