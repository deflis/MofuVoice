use super::presentation::FieldStyle;
use super::*;
use gpui_kit::base::TestSupportExt as _;
impl SettingsView {
    pub(super) fn open_details(
        &mut self,
        section: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pending || self.model.read(cx).locked() || !self.section_enabled(section) {
            return;
        }
        let root = match section {
            "postConversionEq" => "eq",
            s => s,
        };
        if ["eq", "inputEq", "preConversionEq", "deesser"].contains(&root) {
            self.set_value(&format!("/{root}/mode"), "pro".into(), window, cx);
        }
        let weak = cx.weak_entity();
        let section = section.to_string();
        window.open_dialog(cx, move |dialog, _, cx| {
            let owner = weak.upgrade().expect("settings window owns its modal");
            let view = owner.read(cx);
            let model = view.model.read(cx);
            let title = SECTIONS
                .iter()
                .find(|(id, _)| *id == section)
                .map(|(_, label)| *label)
                .unwrap();
            dialog
                .title(format!(
                    "{} · {}",
                    model.text(title),
                    model.text("詳細設定")
                ))
                .width(px(650.))
                .overlay_closable(false)
                .child(view.render_details(&section, owner.clone(), cx))
                .footer(
                    Button::new("close-details")
                        .label(model.text("閉じる"))
                        .on_click(|_, w, cx| w.close_dialog(cx)),
                )
        });
        window.refresh();
        cx.notify();
    }
    fn render_details(&self, section: &str, owner: Entity<Self>, cx: &App) -> AnyElement {
        let mut body = div()
            .id("details-content")
            .max_h(px(500.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3();
        let model = self.model.read(cx);
        if section == "deesser" {
            if self.deesser_custom() {
                body = body.child(model.text(
                    "詳細設定で調整した設定を使っています。ここでは抑える強さだけを変えます。",
                ));
            }
            body=body.child(model.text("まだ刺さるときは右へ。声がこもるときは左へ。「さしすせそ」と話し、オン・オフで聞き比べてください。"))
                .child(model.text("高域の音量と、音全体に対する高域の割合を検出し、高域だけを動的に減衰します。圧縮比は4:1、先読みはありません。"));
        }
        if ["inputEq", "preConversionEq", "postConversionEq"].contains(&section) {
            body=body.child(model.text("ピーキングEQです。帯域幅（Q値）が小さいほど広い帯域、大きいほど狭い帯域に効きます。"));
        }
        if section == "noiseGate" && self.value("/noise/thresholdMode") == "auto" {
            body = body.child(model.text("背景ノイズに合わせてしきい値を自動調整します。"));
        }
        let mut paths: Vec<String> = match section {
            "devices" => vec!["/devices/backend", "/devices/frames"],
            "voice" => vec!["/voice/mathPrecision", "/voice/pitchAlgorithm"],
            "deesser" => vec![
                "/deesser/frequencyHz",
                "/deesser/thresholdDb",
                "/deesser/rangeDb",
                "/deesser/attackMs",
                "/deesser/releaseMs",
            ],
            "noiseGate" => vec![
                "/noise/thresholdMode",
                "/noise/thresholdDb",
                "/noise/attackMs",
                "/noise/releaseMs",
            ],
            "autoLevel" => vec![
                "/level/targetDb",
                "/level/maxGainDb",
                "/level/floorDb",
                "/level/responseMs",
            ],
            "postCompressor" => vec![
                "/level/thresholdDb",
                "/level/ratio",
                "/level/attackMs",
                "/level/releaseMs",
            ],
            _ => Vec::new(),
        }
        .into_iter()
        .map(str::to_string)
        .collect();
        if section == "voice" {
            let algorithm = self.value("/voice/pitchAlgorithm");
            let formant = self
                .value("/voice/formantShift")
                .parse::<f64>()
                .unwrap_or(0.);
            let vocoder = voice::uses_vocoder(&algorithm, formant);
            if vocoder {
                paths.extend(
                    ["/voice/analysisWindow", "/voice/synthesisWindow"].map(str::to_string),
                );
            }
            if algorithm.starts_with("td-psola") || algorithm == "fd-psola" {
                paths.extend(
                    [
                        "/voice/psolaPitchEstimator",
                        "/voice/psolaAnalysisWindow",
                        "/voice/psolaSynthesisWindow",
                    ]
                    .map(str::to_string),
                );
            }
        }
        if ["inputEq", "preConversionEq", "postConversionEq"].contains(&section) {
            let root = if section == "postConversionEq" {
                "eq"
            } else {
                section
            };
            paths = self
                .controls
                .iter()
                .filter(|c| c.field.path.starts_with(&format!("/{root}/bands/")))
                .map(|c| c.field.path.clone())
                .collect();
            paths.push(format!("/{root}/outputDb"));
        }
        for path in paths {
            if path == "/noise/thresholdDb" && self.value("/noise/thresholdMode") == "auto" {
                continue;
            }
            body = body.child(self.render_field(&path, FieldStyle::Number, owner.clone(), cx));
        }
        let model = self.model.read(cx);
        if section == "voice" {
            if let Ok(options) = self.options() {
                body = body.child(voice::latency(&options, model));
            }
            for (id, label) in [
                ("voice-windows", "ウィンドウ長を既定値に戻す"),
                ("psola-windows", "PSOLAのウィンドウ長を既定値に戻す"),
            ] {
                let psola = id == "psola-windows";
                let algorithm = self.value("/voice/pitchAlgorithm");
                if (psola && (algorithm.starts_with("td-psola") || algorithm == "fd-psola"))
                    || (!psola
                        && voice::uses_vocoder(
                            &algorithm,
                            self.value("/voice/formantShift")
                                .parse::<f64>()
                                .unwrap_or(0.),
                        ))
                {
                    body = body.child(self.reset_button(id, label, owner.clone(), cx));
                }
            }
            if self.value("/voice/pitchAlgorithm") == "pvsola" {
                body = body.child(model.text("PVSOLAのウィンドウ長は固定です。"));
            }
        }
        if ["inputEq", "preConversionEq", "postConversionEq"].contains(&section) {
            let root = if section == "postConversionEq" {
                "eq"
            } else {
                section
            };
            let gain = self
                .controls
                .iter()
                .filter(|c| {
                    c.field.path.starts_with(&format!("/{root}/bands/"))
                        && c.field.path.ends_with("/gainDb")
                })
                .map(|c| c.field.text.parse::<f64>())
                .collect::<Result<Vec<_>, _>>();
            if let Ok(gains) = gain {
                let headroom: f64 = gains.iter().map(|v| v.max(0.)).sum();
                body = body.child(format!(
                    "{} {:.1} dB",
                    model.text("クリップ防止の出力調整："),
                    -headroom
                ));
            }
            body = body.child(self.reset_button(section, "EQの補正量を0に戻す", owner.clone(), cx));
            let root = root.to_string();
            let remove_root = root.clone();
            let add_owner = owner.clone();
            let remove_owner = owner.clone();
            let count = self
                .controls
                .iter()
                .filter(|c| {
                    c.field.path.starts_with(&format!("/{root}/bands/"))
                        && c.field.path.ends_with("/gainDb")
                })
                .count();
            let locked = model.locked() || self.pending;
            body = body.child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("add-band")
                            .label(model.text("バンドを追加"))
                            .disabled(locked || count >= crate::eq::MAX_BANDS)
                            .on_click(move |_, w, cx| {
                                add_owner.update(cx, |v, cx| v.band_count(&root, true, w, cx))
                            }),
                    )
                    .child(
                        Button::new("remove-band")
                            .label(model.text("バンドを削除"))
                            .disabled(locked || count <= 1)
                            .on_click(move |_, w, cx| {
                                remove_owner
                                    .update(cx, |v, cx| v.band_count(&remove_root, false, w, cx))
                            }),
                    ),
            );
        }
        if ["deesser", "autoLevel", "postCompressor"].contains(&section) {
            body = body.child(self.reset_button(
                section,
                if section == "deesser" {
                    "ディエッサーを標準に戻す"
                } else {
                    "標準に戻す"
                },
                owner,
                cx,
            ));
        }
        if !self.error.is_empty() {
            body = body.child(
                div()
                    .text_color(crate::gui::theme::palette(cx).danger)
                    .child(self.error_text(cx)),
            );
        }
        body.test_support().into_any_element()
    }
    pub(super) fn reset_button(
        &self,
        target: &str,
        label: &str,
        owner: Entity<Self>,
        cx: &App,
    ) -> AnyElement {
        let target = target.to_string();
        let section = match target.as_str() {
            "voice-windows" | "psola-windows" => "voice",
            s => s,
        };
        Button::new(SharedString::from(format!("reset:{target}")))
            .label(self.model.read(cx).text(label))
            .disabled(
                self.model.read(cx).locked() || self.pending || !self.section_enabled(section),
            )
            .on_click(move |_, w, cx| owner.update(cx, |v, cx| v.reset(&target, w, cx)))
            .into_any_element()
    }
    pub(super) fn reset(&mut self, target: &str, w: &mut Window, cx: &mut Context<Self>) {
        if self.model.read(cx).locked() || self.pending {
            return;
        }
        let defaults = draft::fields(&Settings::from_options(&Options::default()));
        let paths: Vec<String> = match target {
            "voice" => vec!["/voice/pitch".into(), "/voice/formantShift".into()],
            "voice-windows" => vec![
                "/voice/analysisWindow".into(),
                "/voice/synthesisWindow".into(),
            ],
            "psola-windows" => vec![
                "/voice/psolaAnalysisWindow".into(),
                "/voice/psolaSynthesisWindow".into(),
            ],
            "deesser" => [
                "frequencyHz",
                "thresholdDb",
                "rangeDb",
                "attackMs",
                "releaseMs",
            ]
            .map(|k| format!("/deesser/{k}"))
            .to_vec(),
            "autoLevel" => ["targetDb", "maxGainDb", "floorDb", "responseMs"]
                .map(|k| format!("/level/{k}"))
                .to_vec(),
            "postCompressor" => ["thresholdDb", "ratio", "attackMs", "releaseMs"]
                .map(|k| format!("/level/{k}"))
                .to_vec(),
            section => {
                let root = if section == "postConversionEq" {
                    "eq"
                } else {
                    section
                };
                self.controls
                    .iter()
                    .filter(|c| {
                        c.field.path.starts_with(&format!("/{root}/"))
                            && (c.field.path.ends_with("/gainDb")
                                || c.field.path.ends_with("/outputDb"))
                    })
                    .map(|c| c.field.path.clone())
                    .collect()
            }
        };
        for path in paths {
            let value = if target == "voice" {
                if path == "/voice/pitch" && self.percent() {
                    "100"
                } else {
                    "0"
                }
                .into()
            } else if ["inputEq", "preConversionEq", "postConversionEq"].contains(&target) {
                "0".into()
            } else {
                defaults
                    .iter()
                    .find(|f| f.path == path)
                    .expect("default field")
                    .text
                    .clone()
            };
            self.set_value(&path, value, w, cx);
        }
        self.refresh_choices(w, cx);
        self.error.clear();
        self.notice.clear();
        w.refresh();
        cx.notify();
    }
}
