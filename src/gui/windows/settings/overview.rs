mod preview;
use super::*;
use gpui_kit::base::TestSupportExt as _;
pub(super) fn render(
    mut content: Stateful<Div>,
    model: &AppModel,
    view: &SettingsView,
    cx: &Context<SettingsView>,
) -> Stateful<Div> {
    let t = |s: &str| model.text(s);
    content = content.child(div().text_lg().child(t("フィルタ構成")))
        .child(t("上から下が音声の処理順です。各チェックボックスで個別に切り替えられ、無効中は詳細を表示しません。図と値は個別設定と共通で、実測メーターではありません。"));
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
        // Validate each stage independently so an unfinished field in another
        // page does not hide otherwise valid configured responses.
        let section = match stage {
            crate::audio::FilterStage::PostConversionEq => "postConversionEq",
            crate::audio::FilterStage::Level => "autoLevel",
            _ => stage.id(),
        };
        let edits = view
            .edits()
            .into_iter()
            .filter(|(path, _)| {
                draft::group(path, section)
                    || (stage == crate::audio::FilterStage::Level && path.starts_with("/level/"))
                    || path == "/devices/sampleRate"
            })
            .collect();
        match draft::parse(&view.base, &edits) {
            Ok(options) => {
                card = card.child(t(if preview::enabled(stage, &options) { "有効" } else { "無効" }));
                let preview = preview::build(stage, &options, &model.preferences);
                for (i, graph) in preview.graphs.into_iter().enumerate() {
                    card = card.child(div()
                        .id(SharedString::from(format!("overview-plot:{}:{i}", stage.id())))
                        .child(plots::render(graph, cx)).test_support());
                }
                card = card.children(preview.values);
            }
            Err(_) => card = card.child(t("設定値が不正または入力途中のため、図を表示できません。個別設定で確認してください。")),
        }
        content = content.child(card);
    }
    content
}
