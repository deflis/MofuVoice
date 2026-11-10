use super::*;
pub(super) fn render(
    mut content: Stateful<Div>,
    model: &AppModel,
    view: &SettingsView,
    cx: &Context<SettingsView>,
) -> Stateful<Div> {
    let t = |s: &str| model.text(s);
    let options = view.options().ok();
    content = content.child(div().text_lg().child(t("フィルタ構成")));
    for stage in crate::audio::FILTER_STAGES {
        let id = match stage.id() {
            "level" => "autoLevel",
            id => id,
        }
        .to_string();
        let label = SECTIONS
            .iter()
            .find(|(s, _)| *s == id)
            .map(|(_, l)| *l)
            .unwrap_or("声の変換");
        let mut card = div()
            .border_1()
            .bg(crate::gui::theme::palette(cx).surface)
            .border_color(crate::gui::theme::palette(cx).border)
            .rounded_md()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                Button::new(SharedString::from(format!("stage-{id}")))
                    .label(format!("↓  {}", t(label)))
                    .on_click(cx.listener(move |v, _, _, cx| {
                        v.model.update(cx, |m, cx| {
                            m.preferences.section = id.clone();
                            cx.notify();
                        })
                    })),
            );
        let toggles: Vec<(&str, &str)> = match stage {
            crate::audio::FilterStage::InputEq => {
                vec![("/inputEq/enabled", "EQ"), ("/noise/rumbleFilter", "HPF")]
            }
            crate::audio::FilterStage::Limiter => vec![("/limiter/enabled", "有効")],
            crate::audio::FilterStage::NoiseGate => vec![("/noise/enabled", "有効")],
            crate::audio::FilterStage::Dereverb => vec![("/dereverb/enabled", "有効")],
            crate::audio::FilterStage::Deesser => vec![("/deesser/enabled", "有効")],
            crate::audio::FilterStage::Compressor => vec![("/compressor/enabled", "有効")],
            crate::audio::FilterStage::PreConversionEq => {
                vec![("/preConversionEq/enabled", "有効")]
            }
            crate::audio::FilterStage::Voice => vec![("/voice/enabled", "有効")],
            crate::audio::FilterStage::PostConversionEq => vec![("/eq/enabled", "有効")],
            crate::audio::FilterStage::Level => vec![
                ("/level/enabled", "音量の自動調整"),
                ("/level/compressorEnabled", "コンプレッサー"),
            ],
            crate::audio::FilterStage::Reverb => vec![("/reverb/enabled", "有効")],
        };
        let mut switches = div().flex().gap_2();
        for (path, label) in toggles {
            let enabled = view.value(path) == "true";
            switches = switches.child(
                Button::new(SharedString::from(format!("overview:{path}")))
                    .label(t(label))
                    .selected(enabled)
                    .disabled(model.locked() || view.pending)
                    .on_click(cx.listener(move |v, _, window, cx| {
                        v.changed(path, (!enabled).to_string(), window, cx)
                    })),
            );
        }
        card = card.child(switches);
        if let Some(options) = options.as_ref() {
            let enabled = match stage {
                crate::audio::FilterStage::InputEq => {
                    options.input_eq.enabled || options.rumble_filter
                }
                crate::audio::FilterStage::Limiter => options.limiter.enabled,
                crate::audio::FilterStage::NoiseGate => options.gate,
                crate::audio::FilterStage::Dereverb => options.dereverb.enabled,
                crate::audio::FilterStage::Deesser => options.deesser,
                crate::audio::FilterStage::Compressor => options.compressor.enabled,
                crate::audio::FilterStage::PreConversionEq => options.pre_conversion_eq.enabled,
                crate::audio::FilterStage::Voice => options.voice_enabled,
                crate::audio::FilterStage::PostConversionEq => options.eq.enabled,
                crate::audio::FilterStage::Level => {
                    options.level.enabled || options.level.compressor_enabled
                }
                crate::audio::FilterStage::Reverb => options.reverb.enabled,
            };
            card = card.child(t(if enabled { "有効" } else { "無効" }));
            let eq = match stage {
                crate::audio::FilterStage::InputEq => Some(&options.input_eq),
                crate::audio::FilterStage::PreConversionEq => Some(&options.pre_conversion_eq),
                crate::audio::FilterStage::PostConversionEq => Some(&options.eq),
                _ => None,
            };
            if let Some(eq) = eq {
                card = card.child(plots::eq_plot(eq, options.sample_rate, cx));
            }
        }
        content = content.child(card);
    }
    content
}
