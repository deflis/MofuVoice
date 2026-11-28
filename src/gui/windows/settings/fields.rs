use crate::{
    backend::BackendKind,
    gui::{
        draft::{DeviceSlot, GateThresholdMode, PitchUnit},
        preferences::Preferences,
    },
    pitch::{MathPrecision, PitchAlgorithm, PitchEstimator},
};
use gpui_kit::{component::searchable_list::SearchableListItem, SharedString};

pub(super) const ANALYSIS_WINDOWS: [usize; 4] = [1024, 2048, 4096, 8192];
pub(super) const SYNTHESIS_WINDOWS: [usize; 4] = [256, 512, 1024, 2048];
pub(super) const SAMPLE_RATES: [u32; 2] = [44100, 48000];

#[derive(Clone)]
pub(super) struct Choice<T: Clone + PartialEq> {
    pub value: T,
    pub label: SharedString,
}

impl<T: Clone + PartialEq> SearchableListItem for Choice<T> {
    type Value = T;
    fn title(&self) -> SharedString {
        self.label.clone()
    }
    fn value(&self) -> &T {
        &self.value
    }
}

pub(super) fn backend_choices(prefs: &Preferences) -> Vec<Choice<BackendKind>> {
    [BackendKind::Auto, BackendKind::Cpal, BackendKind::Asio]
        .into_iter()
        .filter(|kind| kind.available())
        .map(|value| {
            let label = match value {
                BackendKind::Auto => prefs.text("自動"),
                BackendKind::Cpal => prefs.text("標準"),
                BackendKind::Asio => "ASIO".into(),
            };
            Choice {
                value,
                label: label.into(),
            }
        })
        .collect()
}

pub(super) fn device_choices(
    slot: DeviceSlot,
    items: &[String],
    prefs: &Preferences,
    resolved_backend: BackendKind,
    missing: Option<&str>,
) -> Vec<Choice<String>> {
    items
        .iter()
        .map(|value| {
            let label = if missing == Some(value.as_str()) {
                format!("{value}{}", prefs.text("（未検出）"))
            } else if value.is_empty() {
                let key = match slot {
                    DeviceSlot::Preview => "なし",
                    DeviceSlot::Input if resolved_backend == BackendKind::Asio => "デバイスを選択",
                    DeviceSlot::Input | DeviceSlot::Output => "システムの既定",
                };
                prefs.text(key)
            } else {
                value.clone()
            };
            Choice {
                value: value.clone(),
                label: label.into(),
            }
        })
        .collect()
}

pub(super) fn sample_rate_choices(current: u32) -> Vec<Choice<u32>> {
    let mut values = SAMPLE_RATES.to_vec();
    if !values.contains(&current) {
        values.push(current);
    }
    values
        .into_iter()
        .map(|value| Choice {
            value,
            label: value.to_string().into(),
        })
        .collect()
}

pub(super) fn pitch_unit_choices(prefs: &Preferences) -> Vec<Choice<PitchUnit>> {
    PitchUnit::ALL
        .into_iter()
        .map(|value| Choice {
            value,
            label: prefs.text(value.label()).into(),
        })
        .collect()
}

pub(super) fn math_precision_choices(prefs: &Preferences) -> Vec<Choice<MathPrecision>> {
    [MathPrecision::Standard, MathPrecision::Accurate]
        .into_iter()
        .map(|value| {
            let key = match value {
                MathPrecision::Standard => "標準（高速近似）",
                MathPrecision::Accurate => "精度優先",
            };
            Choice {
                value,
                label: prefs.text(key).into(),
            }
        })
        .collect()
}

pub(super) fn pitch_algorithm_choices(prefs: &Preferences) -> Vec<Choice<PitchAlgorithm>> {
    PitchAlgorithm::ALL
        .into_iter()
        .map(|value| {
            let label = match value {
                PitchAlgorithm::PhaseLocked => prefs.text("位相ロック（標準）"),
                PitchAlgorithm::PvsolaSync => prefs.text("スペクトル位相同期"),
                PitchAlgorithm::Pvsola => "PVSOLA".into(),
                PitchAlgorithm::TdPsola => prefs.text("TD-PSOLA＋フェーズボコーダー"),
                PitchAlgorithm::TdPsolaResampled => prefs.text("TD-PSOLA＋リサンプリング"),
                PitchAlgorithm::FdPsola => "FD-PSOLA".into(),
            };
            Choice {
                value,
                label: label.into(),
            }
        })
        .collect()
}

pub(super) fn pitch_estimator_choices(prefs: &Preferences) -> Vec<Choice<PitchEstimator>> {
    PitchEstimator::ALL
        .into_iter()
        .map(|value| {
            let label = match value {
                PitchEstimator::Yin => prefs.text("YIN（標準）"),
                PitchEstimator::Mpm => "MPM".into(),
                PitchEstimator::MpmFft => prefs.text("MPM＋FFT"),
                PitchEstimator::Nccf => "NCCF".into(),
            };
            Choice {
                value,
                label: label.into(),
            }
        })
        .collect()
}

pub(super) fn analysis_window_choices(current: usize) -> Vec<Choice<usize>> {
    let mut values = ANALYSIS_WINDOWS.to_vec();
    if !values.contains(&current) {
        values.push(current);
    }
    values
        .into_iter()
        .map(|value| Choice {
            value,
            label: value.to_string().into(),
        })
        .collect()
}

pub(super) fn synthesis_window_choices(analysis: usize, current: usize) -> Vec<Choice<usize>> {
    let mut values: Vec<usize> = SYNTHESIS_WINDOWS
        .into_iter()
        .filter(|&size| size <= analysis)
        .collect();
    if !values.contains(&current) {
        values.push(current);
    }
    values
        .into_iter()
        .map(|value| Choice {
            value,
            label: value.to_string().into(),
        })
        .collect()
}

pub(super) fn gate_threshold_mode_choices(prefs: &Preferences) -> Vec<Choice<GateThresholdMode>> {
    GateThresholdMode::ALL
        .into_iter()
        .map(|value| Choice {
            value,
            label: prefs.text(value.label()).into(),
        })
        .collect()
}
