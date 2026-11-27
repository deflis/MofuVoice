use crate::{cli::Options, gui::model::AppModel, pitch::PitchAlgorithm};

pub(super) fn display_percent(text: &str) -> String {
    match text.parse::<f64>().ok().filter(|n| n.is_finite()) {
        Some(value) => format!("{value:.4}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .into(),
        None => text.into(),
    }
}

pub(super) fn uses_vocoder(algorithm: PitchAlgorithm, formant: f64) -> bool {
    matches!(
        algorithm,
        PitchAlgorithm::PhaseLocked | PitchAlgorithm::PvsolaSync
    ) || (matches!(algorithm, PitchAlgorithm::TdPsola | PitchAlgorithm::FdPsola) && formant != 0.)
}

pub(super) fn uses_psola(algorithm: PitchAlgorithm) -> bool {
    matches!(
        algorithm,
        PitchAlgorithm::TdPsola | PitchAlgorithm::TdPsolaResampled | PitchAlgorithm::FdPsola
    )
}

fn latency_text(options: &Options, text: impl Fn(&str) -> String) -> String {
    let configured = options.pitch.latency();
    let converting = options.voice_enabled
        && (options.pitch.semitones != 0. || options.pitch.formant_shift != 0.);
    let active = if converting { configured } else { 0 };
    let ms = |samples: usize| samples as f64 * 1000. / options.sample_rate as f64;
    let note = text("入出力バッファ・ハードウェア遅延を含みません");
    // Neutral pitch/formant bypass the wet path, but still show the selected
    // configuration's conversion delay so window/algorithm choices can be compared.
    if options.voice_enabled && !converting && configured > 0 {
        format!(
            "{}: {:.2} ms（{} {:.2} ms）({})",
            text("DSP遅延"),
            ms(active),
            text("変換時"),
            ms(configured),
            note
        )
    } else {
        format!("{}: {:.2} ms ({})", text("DSP遅延"), ms(active), note)
    }
}

pub(super) fn latency(options: &Options, model: &AppModel) -> String {
    latency_text(options, |key| model.text(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pitch::{PitchAlgorithm, PitchSettings};

    fn options(semitones: f64, formant_shift: f64, algorithm: PitchAlgorithm) -> Options {
        Options {
            voice_enabled: true,
            sample_rate: 48_000,
            pitch: PitchSettings {
                semitones,
                formant_shift,
                algorithm,
                ..PitchSettings::default()
            },
            ..Options::default()
        }
    }

    #[test]
    fn latency_lists_configured_delay_while_pitch_and_formant_are_neutral() {
        let bypass = options(0., 0., PitchAlgorithm::PhaseLocked);
        let configured_ms = bypass.pitch.latency() as f64 * 1000. / bypass.sample_rate as f64;
        assert!(configured_ms > 0.);
        let text = latency_text(&bypass, |key| key.into());
        assert!(text.contains("0.00 ms（変換時"));
        assert!(text.contains(&format!("{configured_ms:.2} ms）")));

        let active = options(7., 0., PitchAlgorithm::PhaseLocked);
        let active_text = latency_text(&active, |key| key.into());
        assert!(!active_text.contains("変換時"));
        assert!(active_text.starts_with(&format!("DSP遅延: {configured_ms:.2} ms ")));
    }
}
