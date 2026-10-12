//! Causal LPC source/filter separation. No lookahead or callback allocation.
//! Reflection coefficients are interpolated in a lattice, never as IIR polynomials.
use crate::RATE;

pub const ORDER: usize = 32;
pub const WINDOW: usize = 1024;
pub const HOP: usize = 128;
const REFLECTION_LIMIT: f64 = 0.995;

pub struct FormantCorrector {
    history: Box<[f64; WINDOW]>,
    window: Box<[f64; WINDOW]>,
    scratch: Box<[f64; WINDOW]>,
    lag_taper: [f64; ORDER * 2 + 1],
    write: usize,
    until_update: usize,
    reflection: [f64; ORDER],
    target: [f64; ORDER],
    step: [f64; ORDER],
    analysis: [f64; ORDER + 1],
    synthesis: [f64; ORDER + 1],
    shift_ratio: f64,
    synthesis_reflection: [f64; ORDER],
    synthesis_target: [f64; ORDER],
    synthesis_step: [f64; ORDER],
}

impl Default for FormantCorrector {
    fn default() -> Self {
        let mut window = Box::new([0.0; WINDOW]);
        for (n, value) in window.iter_mut().enumerate() {
            *value = 0.5 - 0.5 * (std::f64::consts::TAU * n as f64 / (WINDOW - 1) as f64).cos();
        }
        let lag_taper = std::array::from_fn(|lag| {
            (-0.5 * (std::f64::consts::TAU * 35.0 * lag as f64 / RATE as f64).powi(2)).exp()
        });
        Self {
            history: Box::new([0.0; WINDOW]),
            window,
            scratch: Box::new([0.0; WINDOW]),
            lag_taper,
            write: 0,
            until_update: HOP,
            reflection: [0.0; ORDER],
            target: [0.0; ORDER],
            step: [0.0; ORDER],
            analysis: [0.0; ORDER + 1],
            synthesis: [0.0; ORDER + 1],
            shift_ratio: 1.0,
            synthesis_reflection: [0.0; ORDER],
            synthesis_target: [0.0; ORDER],
            synthesis_step: [0.0; ORDER],
        }
    }
}

impl FormantCorrector {
    /// Ratio is validated on the control side, before starting audio.
    pub fn new(shift_ratio: f64) -> Self {
        assert!(shift_ratio.is_finite() && (0.5..=2.0).contains(&shift_ratio));
        Self {
            shift_ratio,
            ..Self::default()
        }
    }

    fn estimate(&mut self) {
        for n in 0..WINDOW {
            self.scratch[n] = self.history[(self.write + n) % WINDOW] * self.window[n];
        }
        let mut correlation = [0.0; ORDER + 1];
        autocorrelation(&self.scratch, &mut correlation);
        self.target.fill(0.0);
        if correlation[0] > WINDOW as f64 * 1e-12 {
            // Diagonal loading and lag taper broaden very narrow resonances,
            // reducing harmonic fitting and excessive synthesis gain.
            correlation[0] *= 1.0001;
            for (lag, value) in correlation.iter_mut().enumerate().skip(1) {
                *value *= self.lag_taper[lag];
            }
            let mut coefficients = [0.0; ORDER + 1];
            coefficients[0] = 1.0;
            let mut error = correlation[0];
            for order in 1..=ORDER {
                let mut sum = correlation[order];
                for j in 1..order {
                    sum += coefficients[j] * correlation[order - j];
                }
                let k = (-sum / error).clamp(-REFLECTION_LIMIT, REFLECTION_LIMIT);
                self.target[order - 1] = k;
                let previous = coefficients;
                for j in 1..order {
                    coefficients[j] = previous[j] + k * previous[order - j];
                }
                coefficients[order] = k;
                error *= 1.0 - k * k;
                if error <= correlation[0] * 1e-9 {
                    break;
                }
            }
        }
        for n in 0..ORDER {
            self.step[n] = (self.target[n] - self.reflection[n]) / HOP as f64;
        }
        if self.shift_ratio != 1.0 {
            // Stretch the autocorrelation's lag axis to move the spectral envelope.
            // Only the synthesis filter changes; excitation and pitch stay independent.
            // All scratch storage is fixed-size, including the extra octave of lags.
            let mut extended = [0.0; ORDER * 2 + 1];
            extended[..=ORDER].copy_from_slice(&correlation);
            let last_lag = (ORDER as f64 * self.shift_ratio).ceil() as usize;
            for (lag, value) in extended
                .iter_mut()
                .enumerate()
                .take(last_lag + 1)
                .skip(ORDER + 1)
            {
                *value = self.scratch[lag..]
                    .iter()
                    .zip(&self.scratch[..WINDOW - lag])
                    .fold(0.0, |sum, (&a, &b)| sum + a * b);
                *value *= self.lag_taper[lag];
            }
            let shifted = std::array::from_fn::<_, { ORDER + 1 }, _>(|lag| {
                let position = lag as f64 * self.shift_ratio;
                let whole = position.floor() as usize;
                let fraction = position - whole as f64;
                extended[whole] * (1.0 - fraction) + extended[(whole + 1).min(ORDER * 2)] * fraction
            });
            self.synthesis_target = reflection_from_correlation(&shifted);
            for n in 0..ORDER {
                self.synthesis_step[n] =
                    (self.synthesis_target[n] - self.synthesis_reflection[n]) / HOP as f64;
            }
        }
    }

    /// Input must already be finite and normalized. Called once per input sample.
    pub fn analyze(&mut self, sample: f64) -> f64 {
        self.history[self.write] = sample;
        self.write = (self.write + 1) % WINDOW;
        self.until_update -= 1;
        if self.until_update == 0 {
            self.reflection = self.target;
            self.synthesis_reflection = self.synthesis_target;
            self.estimate();
            self.until_update = HOP;
        }
        for n in 0..ORDER {
            self.reflection[n] += self.step[n];
            self.synthesis_reflection[n] += self.synthesis_step[n];
        }
        let mut forward = sample;
        // A copy preserves every previous-sample backward error during ascending updates.
        let previous = self.analysis;
        for (n, &k) in self.reflection.iter().enumerate() {
            self.analysis[n + 1] = previous[n] + k * forward;
            forward += k * previous[n];
        }
        self.analysis[0] = sample;
        forward
    }

    /// Restore the envelope at the requested formant ratio after pitch processing.
    pub fn synthesize(&mut self, residual: f64) -> f64 {
        let mut forward = residual;
        for n in (0..ORDER).rev() {
            let k = if self.shift_ratio == 1.0 {
                self.reflection[n]
            } else {
                self.synthesis_reflection[n]
            };
            forward -= k * self.synthesis[n];
            self.synthesis[n + 1] = self.synthesis[n] + k * forward;
        }
        self.synthesis[0] = forward;
        // A bounded wet output protects against resonant transient overloads.
        // Keep internal state unclipped so analysis/synthesis remain inverse.
        forward.clamp(-1.0, 1.0)
    }
}

fn reflection_from_correlation(correlation: &[f64; ORDER + 1]) -> [f64; ORDER] {
    let mut reflection = [0.0; ORDER];
    if correlation[0] <= WINDOW as f64 * 1e-12 {
        return reflection;
    }
    let mut coefficients = [0.0; ORDER + 1];
    coefficients[0] = 1.0;
    let mut error = correlation[0];
    for order in 1..=ORDER {
        let mut sum = correlation[order];
        for j in 1..order {
            sum += coefficients[j] * correlation[order - j];
        }
        let k = (-sum / error).clamp(-REFLECTION_LIMIT, REFLECTION_LIMIT);
        reflection[order - 1] = k;
        let previous = coefficients;
        for j in 1..order {
            coefficients[j] = previous[j] + k * previous[order - j];
        }
        coefficients[order] = k;
        error *= 1.0 - k * k;
        if error <= correlation[0] * 1e-9 {
            break;
        }
    }
    reflection
}

/// Accumulate four lags together so their independent dependency chains can run
/// in parallel. Each lag retains the original ascending sample/addition order.
fn autocorrelation(samples: &[f64; WINDOW], correlation: &mut [f64; ORDER + 1]) {
    correlation[0] = samples.iter().fold(0.0, |sum, &x| sum + x * x);
    for lag in (1..=ORDER).step_by(4) {
        let mut sums = [0.0; 4];
        for n in lag..lag + 3 {
            for offset in 0..=n - lag {
                sums[offset] += samples[n] * samples[n - lag - offset];
            }
        }
        for n in lag + 3..WINDOW {
            let x = samples[n];
            sums[0] += x * samples[n - lag];
            sums[1] += x * samples[n - lag - 1];
            sums[2] += x * samples[n - lag - 2];
            sums[3] += x * samples[n - lag - 3];
        }
        correlation[lag..lag + 4].copy_from_slice(&sums);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_and_synthesis_reconstruct_with_moving_coefficients() {
        let mut dsp = FormantCorrector::default();
        for n in 0..48_000 {
            let input = 0.2 * (std::f64::consts::TAU * 730.0 * n as f64 / RATE as f64).sin()
                + 0.1 * (std::f64::consts::TAU * 2100.0 * n as f64 / RATE as f64).sin();
            let residual = dsp.analyze(input);
            let output = dsp.synthesize(residual);
            assert!((input - output).abs() < 1e-9, "{n}: {input} vs {output}");
            assert!(dsp
                .reflection
                .iter()
                .all(|k| k.abs() <= REFLECTION_LIMIT + 1e-12));
        }
    }

    #[test]
    fn silence_returns_to_identity_without_nonfinite_coefficients() {
        let mut dsp = FormantCorrector::default();
        for n in 0..4096 {
            let residual = dsp.analyze(if n < 2048 { 0.5 } else { 0.0 });
            assert!(dsp.synthesize(residual).is_finite());
        }
        assert_eq!(dsp.target, [0.0; ORDER]);
        assert_eq!(dsp.reflection, [0.0; ORDER]);
        assert_eq!(dsp.analyze(0.0), 0.0);
        assert_eq!(dsp.synthesize(0.0), 0.0);
    }
}
