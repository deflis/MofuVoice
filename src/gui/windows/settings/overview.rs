mod preview;
use super::*;
use crate::audio::FilterStage;
use gpui_kit::base::TestSupportExt as _;

pub(super) fn render(
    mut content: Stateful<Div>,
    model: &AppModel,
    view: &SettingsView,
    cx: &Context<SettingsView>,
) -> Stateful<Div> {
    let t = |s: &str| model.text(s);
    content = content.child(div().text_lg().child(t("フィルタ構成"))).child(t(
        "上から下が音声の処理順です。各ボタンで個別に切り替えられ、無効中は詳細を表示しません。図と値は個別設定と共通で、実測メーターではありません。",
    ));
    for stage in crate::audio::FILTER_STAGES {
        let section = Section::from_stage(stage);
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
                Button::new(SharedString::from(format!("stage-{}", section.id())))
                    .label(format!("↓  {}", t(section.label())))
                    .disabled(model.shutting_down())
                    .on_click(cx.listener(move |v, _, _, cx| {
                        let mut changed = false;
                        v.model.update(cx, |m, cx| {
                            if !m.shutting_down() && m.preferences.section != section {
                                m.preferences.section = section;
                                m.quit_save_failed = false;
                                changed = true;
                                cx.notify();
                            }
                        });
                        if changed {
                            v.save_failed = false;
                        }
                    })),
            );
        let toggles: Vec<(ToggleField, &str)> = match stage {
            FilterStage::InputEq => vec![
                (ToggleField::EqEnabled(EqKind::Input), "EQ"),
                (ToggleField::RumbleFilter, "HPF"),
            ],
            FilterStage::Limiter => vec![(ToggleField::LimiterEnabled, "有効")],
            FilterStage::NoiseGate => vec![(ToggleField::NoiseGateEnabled, "有効")],
            FilterStage::Dereverb => vec![(ToggleField::DereverbEnabled, "有効")],
            FilterStage::Deesser => vec![(ToggleField::DeesserEnabled, "有効")],
            FilterStage::Compressor => vec![(ToggleField::CompressorEnabled, "有効")],
            FilterStage::PreConversionEq => {
                vec![(ToggleField::EqEnabled(EqKind::PreConversion), "有効")]
            }
            FilterStage::Voice => vec![(ToggleField::VoiceEnabled, "有効")],
            FilterStage::PostConversionEq => {
                vec![(ToggleField::EqEnabled(EqKind::PostConversion), "有効")]
            }
            FilterStage::Level => vec![
                (ToggleField::LevelEnabled, "音量の自動調整"),
                (ToggleField::PostCompressorEnabled, "コンプレッサー"),
            ],
            FilterStage::Reverb => vec![(ToggleField::ReverbEnabled, "有効")],
        };
        let mut switches = div().flex().gap_2();
        for (field, label) in toggles {
            let enabled = view.draft.toggle(field);
            switches = switches.child(
                Button::new(SharedString::from(format!("overview:{}", field.id())))
                    .label(t(label))
                    .selected(enabled)
                    .disabled(model.locked() || view.pending())
                    .on_click(cx.listener(move |v, _, window, cx| {
                        v.set_toggle(field, !enabled, window, cx)
                    })),
            );
        }
        card = card.child(switches);
        // Validate each stage independently so an unfinished field in another
        // page does not hide otherwise valid configured responses.
        match view.draft.to_stage_options(stage, &view.base) {
            Ok(options) => {
                card = card.child(t(if preview::enabled(stage, &options) {
                    "有効"
                } else {
                    "無効"
                }));
                let preview = preview::build(stage, &options, &model.preferences);
                for (i, graph) in preview.graphs.into_iter().enumerate() {
                    card = card.child(
                        div()
                            .id(SharedString::from(format!(
                                "overview-plot:{}:{i}",
                                stage.id()
                            )))
                            .child(plots::render(graph, cx))
                            .test_support(),
                    );
                }
                card = card.children(preview.values);
            }
            Err(_) => {
                card = card.child(t(
                    "設定値が不正または入力途中のため、図を表示できません。個別設定で確認してください。",
                ))
            }
        }
        content = content.child(card);
    }
    content
}
