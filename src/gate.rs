//! Soft downward-expander noise gate with optional noise-floor threshold tracking.
//! Zero algorithmic delay: no lookahead. Hold time is internal and not exposed.
use crate::dsp::{
    envelope::decay,
    gain::{amplitude, amplitude_db},
};
use crate::RATE;
use std::{error::Error, fmt};

#[cfg(test)]
const RATIO: f64 = 4.0;
#[cfg(test)]
const FLOOR_DB: f64 = -72.0;
// 10^(-72/20), the original minimum expansion gain.
const FLOOR_GAIN: f64 = 0.000251188643150958;
// The original implementation snaps targets >= 0.999 to unity before hold.
const OPEN_RATIO: f64 = 0.999666555493786; // 0.999^(1/3)
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
        self.build_with_sample_rate(RATE)
    }
    pub fn build_with_sample_rate(self, rate: u32) -> Result<NoiseGate, InvalidGate> {
        NoiseGate::with_sample_rate(
            rate,
            self.enabled,
            self.threshold,
            self.attack_ms,
            self.release_ms,
        )
    }
}

/// Downward expansion gain in dB. Attack/release smoothing softens the open/close edge.
#[cfg(test)]
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
    cached_threshold_db: f64,
    inverse_threshold: f64,
    floor_ceiling_db: f64,
    floor_ceiling: f64,
}

impl NoiseGate {
    pub fn new(
        enabled: bool,
        threshold: GateThreshold,
        attack_ms: f64,
        release_ms: f64,
    ) -> Result<Self, InvalidGate> {
        Self::with_sample_rate(RATE, enabled, threshold, attack_ms, release_ms)
    }
    pub fn with_sample_rate(
        rate: u32,
        enabled: bool,
        threshold: GateThreshold,
        attack_ms: f64,
        release_ms: f64,
    ) -> Result<Self, InvalidGate> {
        crate::validate_sample_rate(rate).map_err(|_| InvalidGate)?;
        let attack_ms = validate_time_ms(attack_ms)?;
        let release_ms = validate_time_ms(release_ms)?;
        match threshold {
            GateThreshold::Auto => {}
            GateThreshold::Db(db) if db.is_finite() && (-90.0..=0.0).contains(&db) => {}
            GateThreshold::Db(_) => return Err(InvalidGate),
        }
        let threshold_db = match threshold {
            GateThreshold::Db(db) => db,
            GateThreshold::Auto => AUTO_MIN_THRESHOLD_DB,
        };
        Ok(Self {
            enabled,
            threshold,
            attack: 1.0 - decay(attack_ms, rate as f64),
            release: 1.0 - decay(release_ms, rate as f64),
            hold_samples: (HOLD_MS * 0.001 * rate as f64).round() as u64,
            detect_attack: 1.0 - decay(DETECT_ATTACK_MS, rate as f64),
            detect_release: 1.0 - decay(DETECT_RELEASE_MS, rate as f64),
            floor_fall: 1.0 - decay(FLOOR_FALL_MS, rate as f64),
            floor_rise: 1.0 - decay(FLOOR_RISE_MS, rate as f64),
            envelope: 0.0,
            gain: 1.0,
            hold_remaining: 0,
            noise_floor_db: -60.0,
            floor_ceiling_db: -48.0,
            floor_ceiling: amplitude(-48.0),
            cached_threshold_db: threshold_db,
            inverse_threshold: amplitude(-threshold_db),
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
            self.refresh_floor_ceiling();
        }
    }

    fn refresh_floor_ceiling(&mut self) {
        // Conservative 6 dB buckets let speech skip the logarithm. Recompute
        // only at bucket crossings, not every noise-floor detector update.
        let limit = self.noise_floor_db + AUTO_MARGIN_DB;
        if limit > self.floor_ceiling_db || limit < self.floor_ceiling_db - 6.0 {
            self.floor_ceiling_db = (limit / 6.0).ceil() * 6.0;
            self.floor_ceiling = amplitude(self.floor_ceiling_db);
        }
    }

    fn process_sample(&mut self, input: f64) -> f64 {
        // Treat invalid active samples as silence in both detector and output.
        let input = if input.is_finite() { input } else { 0.0 };
        let level = input.abs();
        if level > self.envelope {
            self.envelope += (level - self.envelope) * self.detect_attack;
        } else {
            self.envelope += (level - self.envelope) * self.detect_release;
        }
        if matches!(self.threshold, GateThreshold::Auto) {
            if self.envelope <= self.floor_ceiling {
                self.update_noise_floor(amplitude_db(self.envelope, 1e-10));
            }
            let threshold_db = self.threshold_db();
            if threshold_db != self.cached_threshold_db {
                self.cached_threshold_db = threshold_db;
                self.inverse_threshold = amplitude(-threshold_db);
            }
        }
        // 10^(3 * (20 log10(envelope) - threshold_db) / 20)
        // equals (envelope / threshold)^3. Preserve the detector's -200 dB
        // floor and the original 0.999 hold trigger, without log/pow per sample.
        let relative = self.envelope.max(1e-10) * self.inverse_threshold;
        let target = if relative >= OPEN_RATIO {
            self.hold_remaining = self.hold_samples;
            1.0
        } else if self.hold_remaining > 0 {
            self.hold_remaining -= 1;
            1.0
        } else {
            (relative * relative * relative).max(FLOOR_GAIN)
        };
        if target > self.gain {
            self.gain += (target - self.gain) * self.attack;
        } else {
            self.gain += (target - self.gain) * self.release;
        }
        input * self.gain
    }

    #[cfg(test)]
    fn process_sample_reference(&mut self, input: f64) -> f64 {
        // Treat invalid active samples as silence in both detector and output.
        let input = if input.is_finite() { input } else { 0.0 };
        let level = input.abs();
        if level > self.envelope {
            self.envelope += (level - self.envelope) * self.detect_attack;
        } else {
            self.envelope += (level - self.envelope) * self.detect_release;
        }
        let level_db = amplitude_db(self.envelope, 1e-10);
        self.update_noise_floor(level_db);
        let threshold_db = self.threshold_db();
        let mut target = amplitude(expansion_gain_db(level_db, threshold_db));
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

    pub fn process_in_place(&mut self, samples: &mut [f64]) {
        if !self.enabled {
            return;
        }
        for sample in samples {
            *sample = self.process_sample(*sample);
        }
    }

    pub fn process_block(&mut self, input: &[f64], output: &mut [f64]) {
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
            *out = self.process_sample(*sample);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_gain_matches_original_db_curve_and_hold_transitions() {
        for threshold in [
            GateThreshold::Db(-90.0),
            GateThreshold::Db(-40.0),
            GateThreshold::Db(0.0),
            GateThreshold::Auto,
        ] {
            for floor in [-60.0, -50.0, -20.0] {
                let mut actual = NoiseGate::new(true, threshold, 0.1, 2000.0).unwrap();
                let mut reference = NoiseGate::new(true, threshold, 0.1, 2000.0).unwrap();
                actual.noise_floor_db = floor;
                actual.refresh_floor_ceiling();
                reference.noise_floor_db = floor;
                for i in 0..240_000 {
                    // Silence, subnormals, invalid input, threshold sweeps and speech.
                    let input = match i / 40_000 {
                        0 => 0.0,
                        1 => f64::from_bits(1),
                        2 => {
                            if i % 2 == 0 {
                                f64::NAN
                            } else {
                                f64::INFINITY
                            }
                        }
                        3 | 4 => {
                            amplitude(-110.0 + (i % 40_000) as f64 * 0.003)
                                * (i as f64 * 0.031).sin()
                        }
                        _ => 0.0,
                    };
                    let expected = reference.process_sample_reference(input);
                    let output = actual.process_sample(input);
                    assert!(
                        (output - expected).abs() < 1e-12,
                        "{threshold:?}, floor={floor}, sample={i}: {output} != {expected}"
                    );
                    assert_eq!(actual.hold_remaining, reference.hold_remaining);
                    assert!((actual.gain - reference.gain).abs() < 1e-12);
                    assert_eq!(actual.noise_floor_db, reference.noise_floor_db);
                }
            }
        }
    }

    #[test]
    fn irregular_blocks_preserve_state() {
        let input: Vec<f64> = (0..48_000)
            .map(|i| amplitude(-80.0 + (i % 24_000) as f64 / 400.0) * (i as f64 * 0.07).sin())
            .collect();
        for threshold in [GateThreshold::Db(-40.0), GateThreshold::Auto] {
            let mut whole = input.clone();
            NoiseGate::new(true, threshold, 5.0, 80.0)
                .unwrap()
                .process_in_place(&mut whole);
            let mut chunks = input.clone();
            let mut gate = NoiseGate::new(true, threshold, 5.0, 80.0).unwrap();
            let mut position = 0;
            for size in [1, 63, 128, 511, 3].into_iter().cycle() {
                if position == chunks.len() {
                    break;
                }
                let end = (position + size).min(chunks.len());
                gate.process_in_place(&mut chunks[position..end]);
                position = end;
            }
            assert_eq!(whole, chunks);
        }
    }

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
    fn invalid_samples_are_silenced_and_follow_the_silence_envelope() {
        let input = [0.5_f64, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.25];
        let clean = [0.5_f64, 0.0, 0.0, 0.0, 0.25];
        let mut expected = [0.0; 5];
        NoiseGate::new(true, GateThreshold::Auto, 5.0, 80.0)
            .unwrap()
            .process_block(&clean, &mut expected);
        let mut output = [0.0; 5];
        NoiseGate::new(true, GateThreshold::Auto, 5.0, 80.0)
            .unwrap()
            .process_block(&input, &mut output);
        assert_eq!(output, expected);
        let mut in_place = input;
        let mut gate = NoiseGate::new(true, GateThreshold::Auto, 5.0, 80.0).unwrap();
        for chunk in in_place.chunks_mut(2) {
            gate.process_in_place(chunk);
        }
        assert_eq!(in_place, expected);

        let mut bypass = input;
        NoiseGate::new(false, GateThreshold::Auto, 5.0, 80.0)
            .unwrap()
            .process_in_place(&mut bypass);
        assert_eq!(bypass.map(f64::to_bits), input.map(f64::to_bits));
    }

    #[test]
    fn silence_stays_near_zero_when_open_threshold_is_high() {
        let mut gate = NoiseGate::new(true, GateThreshold::Db(-20.0), 1.0, 5.0).unwrap();
        let input = vec![0.001_f64; 4_800];
        let mut output = vec![0.0; input.len()];
        gate.process_block(&input, &mut output);
        let tail = &output[output.len() - 1_000..];
        let rms = (tail.iter().map(|s| s * s).sum::<f64>() / tail.len() as f64).sqrt();
        assert!(rms < 0.0002, "rms={rms}");
    }

    #[test]
    fn strong_tone_passes_after_attack() {
        let mut gate = NoiseGate::new(true, GateThreshold::Db(-40.0), 1.0, 20.0).unwrap();
        let input: Vec<f64> = (0..4_800)
            .map(|i| (std::f64::consts::TAU * 440.0 * i as f64 / RATE as f64).sin() * 0.5)
            .collect();
        let mut output = vec![0.0; input.len()];
        gate.process_block(&input, &mut output);
        let start = output.len() - 1_000;
        let err: f64 = input[start..]
            .iter()
            .zip(&output[start..])
            .map(|(a, b)| (a - b).abs())
            .sum::<f64>()
            / 1_000.0;
        assert!(err < 0.02, "mean abs err={err}");
    }

    #[test]
    fn auto_threshold_learns_quiet_floor_and_opens_for_speech() {
        let mut gate = NoiseGate::new(true, GateThreshold::Auto, 2.0, 30.0).unwrap();
        let noise: Vec<f64> = vec![0.0005; 48_000];
        let mut scratch = vec![0.0; noise.len()];
        gate.process_block(&noise, &mut scratch);
        let speech: Vec<f64> = (0..24_000)
            .map(|i| (std::f64::consts::TAU * 220.0 * i as f64 / RATE as f64).sin() * 0.25)
            .collect();
        let mut out = vec![0.0; speech.len()];
        gate.process_block(&speech, &mut out);
        let peak = out[out.len() - 2_000..]
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f64, f64::max);
        assert!(peak > 0.1, "peak={peak}");
        assert!(gate.threshold_db() > gate.noise_floor_db);
    }

    #[test]
    fn zero_additional_latency_on_impulse_when_open() {
        let mut gate = NoiseGate::new(true, GateThreshold::Db(-60.0), 0.1, 0.1).unwrap();
        // Prime the gate open with a loud tone.
        let prime = vec![0.5_f64; 2_000];
        let mut scratch = vec![0.0; prime.len()];
        gate.process_block(&prime, &mut scratch);
        let mut input = vec![0.0_f64; 64];
        input[0] = 1.0;
        let mut output = vec![0.0; 64];
        gate.process_block(&input, &mut output);
        assert!(output[0].abs() > 0.9, "out0={}", output[0]);
    }

    #[test]
    fn auto_suppresses_repeated_quiet_clicks_after_silence() {
        let mut gate = NoiseGate::new(true, GateThreshold::Auto, 5.0, 80.0).unwrap();
        gate.process_in_place(&mut vec![0.0_f64; RATE as usize * 2]);
        let amplitude = amplitude(-52.0);
        let mut clicks = vec![0.0_f64; RATE as usize];
        for start in (0..clicks.len()).step_by(RATE as usize / 10) {
            // Three millisecond alternating transients, ten keystrokes per second.
            for i in 0..144 {
                clicks[start + i] = if i % 2 == 0 { amplitude } else { -amplitude };
            }
        }
        gate.process_in_place(&mut clicks);
        let peak = clicks.iter().map(|x| x.abs()).fold(0.0_f64, f64::max);
        assert!(peak < amplitude * 0.1, "click attenuation <20 dB: {peak}");
        assert_eq!(gate.threshold_db(), AUTO_MIN_THRESHOLD_DB);
        assert_eq!(gate.hold_remaining, 0, "clicks must not trigger hold");
    }

    #[test]
    fn auto_opens_for_quiet_voice_after_silence_and_closes_after_speech() {
        let mut gate = NoiseGate::new(true, GateThreshold::Auto, 5.0, 80.0).unwrap();
        gate.process_in_place(&mut vec![0.0_f64; RATE as usize * 2]);
        let tone_amplitude = amplitude(-34.0);
        let mut voice: Vec<f64> = (0..RATE as usize / 2)
            .map(|i| {
                (std::f64::consts::TAU * 180.0 * i as f64 / RATE as f64).sin() * tone_amplitude
            })
            .collect();
        let original = voice.clone();
        gate.process_in_place(&mut voice);
        // Gain attack must open promptly without buffering the speech onset.
        let onset = (0..960).find(|&i| {
            original[i].abs() > tone_amplitude * 0.5 && voice[i].abs() > original[i].abs() * 0.8
        });
        assert!(onset.is_some(), "voice must open within 20 ms");
        for (input, output) in original[2400..].iter().zip(&voice[2400..]) {
            assert!((input - output).abs() < tone_amplitude * 0.001);
        }
        gate.process_in_place(&mut vec![0.0_f64; RATE as usize]);
        let mut noise = vec![amplitude(-52.0); RATE as usize / 2];
        gate.process_in_place(&mut noise);
        assert!(noise.iter().all(|x| x.abs() < amplitude(-52.0) * 0.1));
    }

    #[test]
    fn manual_threshold_can_still_be_lower_than_auto_minimum() {
        let mut gate = NoiseGate::new(true, GateThreshold::Db(-80.0), 5.0, 80.0).unwrap();
        let mut quiet = vec![amplitude(-65.0); RATE as usize / 2];
        gate.process_in_place(&mut quiet);
        assert_eq!(gate.threshold_db(), -80.0);
        assert!(quiet.last().unwrap().abs() > amplitude(-65.0) * 0.99);
    }
}
