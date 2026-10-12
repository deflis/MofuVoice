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
    lag_taper: [f64; ORDER + 1],
    write: usize,
    until_update: usize,
    reflection: [f64; ORDER],
    target: [f64; ORDER],
    step: [f64; ORDER],
    analysis: [f64; ORDER + 1],
    synthesis: [f64; ORDER + 1],
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
        }
    }
}

impl FormantCorrector {
    fn estimate(&mut self) {
        for n in 0..WINDOW {
            self.scratch[n] = self.history[(self.write + n) % WINDOW] * self.window[n];
        }
        let mut correlation = [0.0; ORDER + 1];
        for (lag, value) in correlation.iter_mut().enumerate() {
            for n in lag..WINDOW {
                *value += self.scratch[n] * self.scratch[n - lag];
            }
        }
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
    }

    /// Input must already be finite and normalized. Called once per input sample.
    pub fn analyze(&mut self, sample: f64) -> f64 {
        self.history[self.write] = sample;
        self.write = (self.write + 1) % WINDOW;
        self.until_update -= 1;
        if self.until_update == 0 {
            self.reflection = self.target;
            self.estimate();
            self.until_update = HOP;
        }
        for n in 0..ORDER {
            self.reflection[n] += self.step[n];
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

    /// Restore the original envelope after the excitation's pitch has changed.
    pub fn synthesize(&mut self, residual: f64) -> f64 {
        let mut forward = residual;
        for n in (0..ORDER).rev() {
            let k = self.reflection[n];
            forward -= k * self.synthesis[n];
            self.synthesis[n + 1] = self.synthesis[n] + k * forward;
        }
        self.synthesis[0] = forward;
        // A bounded wet output protects against resonant transient overloads.
        // Keep internal state unclipped so analysis/synthesis remain inverse.
        forward.clamp(-1.0, 1.0)
    }
}
