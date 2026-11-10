use crate::{cli::Options, gui::model::AppModel};

pub(super) fn display_percent(text: &str) -> String {
    match text.parse::<f64>().ok().filter(|n| n.is_finite()) {
        Some(value) => format!("{value:.4}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .into(),
        None => text.into(),
    }
}
pub(super) fn uses_vocoder(algorithm: &str, formant: f64) -> bool {
    ["phase-locked", "pvsola-sync"].contains(&algorithm)
        || (["td-psola", "fd-psola"].contains(&algorithm) && formant != 0.)
}

pub(super) fn latency(options: &Options, model: &AppModel) -> String {
    let samples = if options.voice_enabled
        && (options.pitch.semitones != 0. || options.pitch.formant_shift != 0.)
    {
        options.pitch.latency()
    } else {
        0
    };
    format!(
        "{}: {:.2} ms ({})",
        model.text("DSP遅延"),
        samples as f64 * 1000. / options.sample_rate as f64,
        model.text("入出力バッファ・ハードウェア遅延を含みません")
    )
}
