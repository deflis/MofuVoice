use super::presentation::FieldStyle;
use super::*;
use crate::audio::FilterStage;
use gpui_kit::base::TestSupportExt as _;

impl SettingsView {
    pub(super) fn open_details(
        &mut self,
        section: Section,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pending() || self.model.read(cx).locked() || !self.draft.section_enabled(section) {
            return;
        }
        let mode_changed = if let Some(kind) = EqKind::from_section(section) {
            let eq = self.draft.eq_mut(kind);
            if eq.mode != UiMode::Pro {
                eq.mode = UiMode::Pro;
                true
            } else {
                false
            }
        } else if section == Section::Deesser && self.draft.deesser_mode != UiMode::Pro {
            self.draft.deesser_mode = UiMode::Pro;
            true
        } else {
            false
        };
        if mode_changed {
            self.save_failed = false;
            self.notice.clear();
            self.notify_host(cx);
        }
        let weak = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let owner = weak.upgrade().expect("settings window owns its modal");
            let view = owner.read(cx);
            let model = view.model.read(cx);
            dialog
                .title(format!(
                    "{} · {}",
                    model.text(section.label()),
                    model.text("詳細設定")
                ))
                .width(px(650.))
                .overlay_closable(false)
                .child(view.render_details(section, owner.clone(), cx))
                .footer(
                    Button::new("close-details")
                        .label(model.text("閉じる"))
                        .on_click(|_, w, cx| w.close_dialog(cx)),
                )
        });
        window.refresh();
        cx.notify();
    }

    fn render_details(&self, section: Section, owner: Entity<Self>, cx: &App) -> AnyElement {
        let mut body = div()
            .id("details-content")
            .max_h(px(500.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3();
        let model = self.model.read(cx);
        match section {
            Section::Devices => {
                body = body
                    .child(self.render_select(SelectField::Backend, cx))
                    .child(self.render_number(NumberField::Frames, FieldStyle::Number, cx));
            }
            Section::Voice => {
                let algorithm = self.draft.pitch_algorithm;
                let formant = self.draft.formant_shift.parse::<f64>().unwrap_or(0.);
                let vocoder = voice::uses_vocoder(algorithm, formant);
                let psola = voice::uses_psola(algorithm);
                body = body
                    .child(self.render_select(SelectField::MathPrecision, cx))
                    .child(self.render_select(SelectField::PitchAlgorithm, cx));
                if vocoder {
                    body = body
                        .child(self.render_select(SelectField::AnalysisWindow, cx))
                        .child(self.render_select(SelectField::SynthesisWindow, cx));
                }
                if psola {
                    body = body
                        .child(self.render_select(SelectField::PsolaPitchEstimator, cx))
                        .child(self.render_select(SelectField::PsolaAnalysisWindow, cx))
                        .child(self.render_select(SelectField::PsolaSynthesisWindow, cx));
                }
                if let Ok(options) = self.draft.to_stage_options(FilterStage::Voice, &self.base) {
                    body = body.child(voice::latency(&options, model));
                }
                if vocoder {
                    body = body.child(self.reset_button(
                        ResetTarget::VoiceWindows,
                        "ウィンドウ長を既定値に戻す",
                        owner.clone(),
                        cx,
                    ));
                }
                if psola {
                    body = body.child(self.reset_button(
                        ResetTarget::PsolaWindows,
                        "PSOLAのウィンドウ長を既定値に戻す",
                        owner.clone(),
                        cx,
                    ));
                }
                if algorithm == PitchAlgorithm::Pvsola {
                    body = body.child(model.text("PVSOLAのウィンドウ長は固定です。"));
                }
            }
            Section::Deesser => {
                if self.deesser_custom() {
                    body = body.child(model.text(
                        "詳細設定で調整した設定を使っています。ここでは抑える強さだけを変えます。",
                    ));
                }
                body = body
                    .child(model.text(
                        "まだ刺さるときは右へ。声がこもるときは左へ。「さしすせそ」と話し、オン・オフで聞き比べてください。",
                    ))
                    .child(model.text(
                        "高域の音量と、音全体に対する高域の割合を検出し、高域だけを動的に減衰します。圧縮比は4:1、先読みはありません。",
                    ))
                    .child(self.render_number(
                        NumberField::DeesserFrequencyHz,
                        FieldStyle::Number,
                        cx,
                    ))
                    .child(self.render_number(
                        NumberField::DeesserThresholdDb,
                        FieldStyle::Number,
                        cx,
                    ))
                    .child(self.render_number(NumberField::DeesserRangeDb, FieldStyle::Number, cx))
                    .child(self.render_number(
                        NumberField::DeesserAttackMs,
                        FieldStyle::Number,
                        cx,
                    ))
                    .child(self.render_number(
                        NumberField::DeesserReleaseMs,
                        FieldStyle::Number,
                        cx,
                    ))
                    .child(self.reset_button(
                        ResetTarget::Deesser,
                        "ディエッサーを標準に戻す",
                        owner,
                        cx,
                    ));
            }
            Section::NoiseGate => {
                if self.draft.gate_threshold_mode == GateThresholdMode::Auto {
                    body = body.child(model.text("背景ノイズに合わせてしきい値を自動調整します。"));
                }
                body = body.child(self.render_select(SelectField::GateThresholdMode, cx));
                if self.draft.gate_threshold_mode == GateThresholdMode::Manual {
                    body = body.child(self.render_number(
                        NumberField::GateThresholdDb,
                        FieldStyle::Number,
                        cx,
                    ));
                }
                body = body
                    .child(self.render_number(NumberField::GateAttackMs, FieldStyle::Number, cx))
                    .child(self.render_number(NumberField::GateReleaseMs, FieldStyle::Number, cx));
            }
            Section::AutoLevel => {
                body = body
                    .child(self.render_number(NumberField::LevelTargetDb, FieldStyle::Number, cx))
                    .child(self.render_number(NumberField::LevelMaxGainDb, FieldStyle::Number, cx))
                    .child(self.render_number(NumberField::LevelFloorDb, FieldStyle::Number, cx))
                    .child(self.render_number(NumberField::LevelResponseMs, FieldStyle::Number, cx))
                    .child(self.reset_button(ResetTarget::AutoLevel, "標準に戻す", owner, cx));
            }
            Section::PostCompressor => {
                body = body
                    .child(self.render_number(
                        NumberField::PostCompressorThresholdDb,
                        FieldStyle::Number,
                        cx,
                    ))
                    .child(self.render_number(
                        NumberField::PostCompressorRatio,
                        FieldStyle::Number,
                        cx,
                    ))
                    .child(self.render_number(
                        NumberField::PostCompressorAttackMs,
                        FieldStyle::Number,
                        cx,
                    ))
                    .child(self.render_number(
                        NumberField::PostCompressorReleaseMs,
                        FieldStyle::Number,
                        cx,
                    ))
                    .child(self.reset_button(ResetTarget::PostCompressor, "標準に戻す", owner, cx));
            }
            Section::InputEq | Section::PreConversionEq | Section::PostConversionEq => {
                let kind = EqKind::from_section(section).expect("EQ section");
                let eq = self.draft.eq(kind);
                body = body.child(model.text(
                    "ピーキングEQです。帯域幅（Q値）が小さいほど広い帯域、大きいほど狭い帯域に効きます。",
                ));
                for index in 0..eq.bands.len() {
                    for field in EqBandField::ALL {
                        body = body.child(self.render_number(
                            NumberField::EqBand(kind, index, field),
                            FieldStyle::Number,
                            cx,
                        ));
                    }
                }
                body = body.child(self.render_number(
                    NumberField::EqOutputDb(kind),
                    FieldStyle::Number,
                    cx,
                ));
                let gains = eq
                    .bands
                    .iter()
                    .map(|band| band.gain_db.parse::<f64>())
                    .collect::<Result<Vec<_>, _>>();
                if let Ok(gains) = gains {
                    let headroom: f64 = gains.iter().map(|v| v.max(0.)).sum();
                    body = body.child(format!(
                        "{} {:.1} dB",
                        model.text("クリップ防止の出力調整："),
                        -headroom
                    ));
                }
                body = body.child(self.reset_button(
                    ResetTarget::Eq(kind),
                    "EQの補正量を0に戻す",
                    owner.clone(),
                    cx,
                ));
                let count = eq.bands.len();
                let locked = model.locked() || self.pending();
                let add_owner = owner.clone();
                let remove_owner = owner;
                body = body.child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            Button::new("add-band")
                                .label(model.text("バンドを追加"))
                                .disabled(locked || count >= crate::eq::MAX_BANDS)
                                .on_click(move |_, w, cx| {
                                    add_owner.update(cx, |v, cx| v.band_count(kind, true, w, cx))
                                }),
                        )
                        .child(
                            Button::new("remove-band")
                                .label(model.text("バンドを削除"))
                                .disabled(locked || count <= 1)
                                .on_click(move |_, w, cx| {
                                    remove_owner
                                        .update(cx, |v, cx| v.band_count(kind, false, w, cx))
                                }),
                        ),
                );
            }
            _ => {}
        }
        if self.error.is_some() {
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
        target: ResetTarget,
        label: &str,
        owner: Entity<Self>,
        cx: &App,
    ) -> AnyElement {
        Button::new(SharedString::from(format!("reset:{}", target.id())))
            .label(self.model.read(cx).text(label))
            .disabled(
                self.model.read(cx).locked()
                    || self.pending()
                    || !self.draft.section_enabled(target.section()),
            )
            .on_click(move |_, w, cx| owner.update(cx, |v, cx| v.reset(target, w, cx)))
            .into_any_element()
    }

    pub(super) fn reset(&mut self, target: ResetTarget, w: &mut Window, cx: &mut Context<Self>) {
        if self.model.read(cx).locked() || self.pending() {
            return;
        }
        let defaults = Options::default();
        match target {
            ResetTarget::Voice => {
                match self.draft.pitch_unit {
                    PitchUnit::Percent => {
                        self.draft.pitch = "100".into();
                        self.draft.pitch_exact_semitones = Some(0.0);
                    }
                    PitchUnit::Semitones => {
                        self.draft.pitch = "0".into();
                        self.draft.pitch_exact_semitones = None;
                    }
                }
                self.sync_number_widgets(NumberField::Pitch, w, cx);
                self.draft
                    .set_number_text(NumberField::FormantShift, "0".into());
                self.sync_number_widgets(NumberField::FormantShift, w, cx);
            }
            ResetTarget::VoiceWindows => {
                self.draft.analysis_window = defaults.pitch.windows.analysis;
                self.draft.synthesis_window = defaults.pitch.windows.synthesis;
            }
            ResetTarget::PsolaWindows => {
                self.draft.psola_analysis_window = defaults.pitch.psola_windows.analysis;
                self.draft.psola_synthesis_window = defaults.pitch.psola_windows.synthesis;
            }
            ResetTarget::Deesser => {
                let d = defaults.deesser_parameters;
                for (field, value) in [
                    (NumberField::DeesserFrequencyHz, d.frequency_hz),
                    (NumberField::DeesserThresholdDb, d.threshold_db),
                    (NumberField::DeesserRangeDb, d.range_db),
                    (NumberField::DeesserAttackMs, d.attack_ms),
                    (NumberField::DeesserReleaseMs, d.release_ms),
                ] {
                    self.draft.set_number_text(field, value.to_string());
                    self.sync_number_widgets(field, w, cx);
                }
            }
            ResetTarget::AutoLevel => {
                let l = defaults.level;
                for (field, value) in [
                    (NumberField::LevelTargetDb, l.target_db),
                    (NumberField::LevelMaxGainDb, l.max_gain_db),
                    (NumberField::LevelFloorDb, l.floor_db),
                    (NumberField::LevelResponseMs, l.response_ms),
                ] {
                    self.draft.set_number_text(field, value.to_string());
                    self.sync_number_widgets(field, w, cx);
                }
            }
            ResetTarget::PostCompressor => {
                let l = defaults.level;
                for (field, value) in [
                    (NumberField::PostCompressorThresholdDb, l.threshold_db),
                    (NumberField::PostCompressorRatio, l.ratio),
                    (NumberField::PostCompressorAttackMs, l.attack_ms),
                    (NumberField::PostCompressorReleaseMs, l.release_ms),
                ] {
                    self.draft.set_number_text(field, value.to_string());
                    self.sync_number_widgets(field, w, cx);
                }
            }
            ResetTarget::Eq(kind) => {
                let band_count = self.draft.eq(kind).bands.len();
                for index in 0..band_count {
                    let field = NumberField::EqBand(kind, index, EqBandField::GainDb);
                    self.draft.set_number_text(field, "0".into());
                    self.sync_number_widgets(field, w, cx);
                }
                let output = NumberField::EqOutputDb(kind);
                self.draft.set_number_text(output, "0".into());
                self.sync_number_widgets(output, w, cx);
            }
        }
        self.refresh_choices(w, cx);
        self.error = None;
        self.notice.clear();
        self.save_failed = false;
        w.refresh();
        self.notify_host(cx);
        cx.notify();
    }
}
