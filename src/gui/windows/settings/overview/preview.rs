use crate::{
    audio::FilterStage,
    cli::Options,
    gate::GateThreshold,
    gui::{
        plots::{self, Diagram},
        preferences::Preferences,
    },
};

#[derive(Default, Debug, PartialEq)]
pub struct Preview {
    pub graphs: Vec<Diagram>,
    pub values: Vec<String>,
}
fn num(value: f64) -> String {
    let s = format!("{value:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}
fn timing(p: &Preferences, attack: f64, release: f64) -> String {
    format!(
        "{} {} ms · {} {} ms",
        p.text("アタック"),
        num(attack),
        p.text("リリース"),
        num(release)
    )
}
pub fn enabled(stage: FilterStage, o: &Options) -> bool {
    match stage {
        FilterStage::InputEq => o.input_eq.enabled || o.rumble_filter,
        FilterStage::Limiter => o.limiter.enabled,
        FilterStage::NoiseGate => o.gate,
        FilterStage::Dereverb => o.dereverb.enabled,
        FilterStage::Deesser => o.deesser,
        FilterStage::Compressor => o.compressor.enabled,
        FilterStage::PreConversionEq => o.pre_conversion_eq.enabled,
        FilterStage::Voice => o.voice_enabled,
        FilterStage::PostConversionEq => o.eq.enabled,
        FilterStage::Level => o.level.enabled || o.level.compressor_enabled,
        FilterStage::Reverb => o.reverb.enabled,
    }
}
fn transfer(threshold: f64, ratio: f64, soft: bool, p: &Preferences, description: &str) -> Diagram {
    let mut graph = Diagram::new(
        p.text(description),
        p.text("入力 −60 → 0 dBFS"),
        p.text("出力 dBFS"),
    );
    graph.references.push(vec![(36., 86.), (308., 22.)]);
    graph.paths.push(
        (0..=120)
            .map(|i| {
                let input = -60. + i as f64 / 2.;
                let over = input - threshold;
                let reduction = if soft && (-3. ..3.).contains(&over) {
                    (1. - 1. / ratio) * (over + 3.).powi(2) / 12.
                } else {
                    (1. - 1. / ratio) * over.max(0.)
                };
                (
                    36. + i as f64 / 120. * 272.,
                    86. - (input - reduction + 60.) / 60. * 64.,
                )
            })
            .collect(),
    );
    graph
}

/// Previews are configured responses or explicitly labelled schematics, never live DSP measurements.
pub fn build(stage: FilterStage, o: &Options, p: &Preferences) -> Preview {
    let t = |s: &str| p.text(s);
    let mut out = Preview::default();
    if !enabled(stage, o) {
        return out;
    }
    match stage {
        FilterStage::InputEq | FilterStage::PreConversionEq | FilterStage::PostConversionEq => {
            let (eq, rumble) = match stage {
                FilterStage::InputEq => (&o.input_eq, o.rumble_filter),
                FilterStage::PreConversionEq => (&o.pre_conversion_eq, false),
                _ => (&o.eq, false),
            };
            let mut graph = plots::eq_diagram(
                eq,
                o.sample_rate,
                rumble,
                t(if stage == FilterStage::InputEq {
                    "図は有効なHPFとEQの合成特性です。"
                } else {
                    "図はEQの設定周波数応答です。"
                }),
            );
            graph.x_label = t("20 Hz → 20 kHz（対数）");
            out.graphs.push(graph);
            if rumble {
                out.values.push(t("低域ノイズ対策: 80 Hz HPF"));
            }
            if eq.enabled {
                out.values.push(
                    eq.bands
                        .iter()
                        .map(|b| {
                            format!(
                                "{} Hz: {} dB / Q {}",
                                num(b.frequency),
                                num(b.gain_db),
                                num(b.q)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" · "),
                );
                out.values.push(format!(
                    "{} {} dB · {} {} dB · {} Hz",
                    t("出力調整"),
                    num(eq.output_db),
                    t("ヘッドルーム"),
                    num(eq.bands.iter().map(|b| b.gain_db.max(0.)).sum()),
                    o.sample_rate
                ));
                out.values.push(t("出力調整と内部ヘッドルームを含みます。"));
            }
        }
        FilterStage::NoiseGate => {
            let attack_x = 68. + o.gate_attack_ms / (o.gate_attack_ms + o.gate_release_ms) * 150.;
            let mut graph = Diagram::new(t("ゲートが開いてから閉じるまでの時間応答の模式図。実際の検出レベルやゲインではありません。"), t("時間（模式図）"), t("開閉"));
            graph.paths.push(vec![
                (36., 86.),
                (68., 86.),
                (attack_x, 24.),
                (attack_x + 50., 24.),
                (268., 86.),
                (308., 86.),
            ]);
            out.graphs.push(graph);
            out.values.push(match o.gate_threshold {
                GateThreshold::Auto => t("しきい値: 自動推定（実測値は表示しません）"),
                GateThreshold::Db(db) => format!("{} {} dBFS", t("しきい値"), num(db)),
            });
            out.values
                .push(timing(p, o.gate_attack_ms, o.gate_release_ms));
        }
        FilterStage::Deesser => {
            let s = o.deesser_parameters;
            let x = 36. + (s.frequency_hz / 1000.).log10() / 20f64.log10() * 272.;
            let mut graph = Diagram::new(
                t("歯擦音の検出帯域と最大抑制量の模式図。固定EQの周波数応答ではありません。"),
                t("1 → 20 kHz（対数・模式図）"),
                t("検出帯域"),
            );
            graph.areas.push([x, 22., 308. - x, 64.]);
            graph.paths.push(vec![(x, 22.), (x, 86.)]);
            out.graphs.push(graph);
            out.values.push(format!(
                "{} {} dB · {} {} Hz · {} {} dBFS",
                t("最大"),
                num(s.range_db),
                t("検出"),
                num(s.frequency_hz),
                t("しきい値"),
                num(s.threshold_db)
            ));
            out.values.push(timing(p, s.attack_ms, s.release_ms));
            out.values
                .push(t("声変換の前に歯擦音を抑えます。図は検出設定の目安です。"));
        }
        FilterStage::Voice => {
            let x = |shift: f64| 36. + (shift + 12.) / 24. * 272.;
            let s = o.pitch;
            let mut graph = Diagram::new(t("ピッチと独立フォルマント調整の相対移動量。入力音声の基本周波数ではありません。"), t("−12 → +12 半音（相対設定）"), format!("{} / {}", t("ピッチ"), t("フォルマント")));
            graph.references.push(vec![(172., 16.), (172., 86.)]);
            graph.paths = vec![
                vec![(172., 38.), (x(s.semitones), 38.)],
                vec![(172., 68.), (x(s.formant_shift), 68.)],
            ];
            graph.markers = vec![(x(s.semitones), 38.), (x(s.formant_shift), 68.)];
            out.graphs.push(graph);
            out.values.push(format!(
                "{} {} {} ({}%) · {} {} {}",
                t("ピッチ"),
                num(s.semitones),
                t("半音"),
                num(100. * 2f64.powf(s.semitones / 12.)),
                t("フォルマント"),
                num(s.formant_shift),
                t("半音")
            ));
            out.values.push(format!(
                "{} {} · {} {}",
                t("方式"),
                s.algorithm.name(),
                t("フォルマント独立調整"),
                t(if s.preserve_formants {
                    "有効"
                } else {
                    "無効"
                })
            ));
        }
        FilterStage::Compressor => {
            let s = o.compressor;
            out.graphs.push(transfer(
                s.threshold_db,
                s.ratio,
                false,
                p,
                "変換前コンプレッサーのハードニー設定静特性。検出と時間応答を含みません。",
            ));
            out.values.push(format!(
                "{} {} dBFS · {} {}:1",
                t("しきい値"),
                num(s.threshold_db),
                t("比率"),
                num(s.ratio)
            ));
            out.values.push(timing(p, s.attack_ms, s.release_ms));
            out.values
                .push(t("図は設定静特性です。実測の抑制量ではありません。"));
        }
        FilterStage::Limiter => {
            let s = o.limiter;
            out.graphs.push(transfer(
                s.ceiling_db,
                f64::INFINITY,
                false,
                p,
                "リミッターの設定上限による静特性。リリースの時間応答は含みません。",
            ));
            out.values.push(format!(
                "{} {} dBFS · {} {} ms",
                t("上限"),
                num(s.ceiling_db),
                t("リリース"),
                num(s.release_ms)
            ));
            out.values.push(t("前段のサンプルピークを抑えます。後段の処理後のピークを保証するものではありません。"));
        }
        FilterStage::Dereverb => {
            let s = o.dereverb;
            let mut graph = Diagram::new(t("残響末尾を抑える強さの設定目安。直接音と反響の分離や実測抑制量を表すものではありません。"),t("抑制強度 0 → 100%（模式図）"),t("設定量"));
            graph.areas.push([36., 35., 272., 30.]);
            graph
                .paths
                .push(vec![(36., 50.), (36. + s.amount * 272., 50.)]);
            graph.markers.push((36. + s.amount * 272., 50.));
            out.graphs.push(graph);
            out.values.push(format!(
                "{} {}% · {} {} ms",
                t("抑制強度"),
                num(s.amount * 100.),
                t("減衰時間"),
                num(s.decay_ms)
            ));
            out.values.push(t("入力の包絡に基づき音の末尾を弱める簡易方式です。直接音や小声にも影響する場合があります。"));
        }
        FilterStage::Level => {
            let s = o.level;
            if s.enabled {
                out.values.push(format!(
                    "{} {} dBFS · {} {} dB · {} {} dBFS · {} {} ms",
                    t("目標"),
                    num(s.target_db),
                    t("増幅上限"),
                    num(s.max_gain_db),
                    t("無音判定"),
                    num(s.floor_db),
                    t("応答"),
                    num(s.response_ms)
                ));
            }
            if s.compressor_enabled {
                out.graphs.push(transfer(s.threshold_db,s.ratio,true,p,"コンプレッサー単体の設定静特性。6 dBソフトニーを含みます。時間応答と内部ピークガードの効果は含みません。"));
                out.values.push(format!(
                    "{} {} dBFS · {} {}:1",
                    t("圧縮しきい値"),
                    num(s.threshold_db),
                    t("比率"),
                    num(s.ratio)
                ));
                out.values.push(timing(p, s.attack_ms, s.release_ms));
            }
            out.values.push(t("図は圧縮の設定静特性です。有効な音量処理が動作する場合、内部ピークガード（−1 dBFS）を適用します。"));
        }
        FilterStage::Reverb => {
            let s = o.reverb;
            let duration = s.pre_delay_ms + s.decay_ms;
            let x = |time: f64| 36. + time / duration * 272.;
            let mut graph = Diagram::new(
                t("プリディレイと残響減衰の設定イメージ。実測インパルス応答ではありません。"),
                format!("0 → {} ms ({})", num(duration), t("模式図")),
                t("残響の目安"),
            );
            graph
                .references
                .push(vec![(x(s.pre_delay_ms), 22.), (x(s.pre_delay_ms), 86.)]);
            graph.paths.push(
                (0..=80)
                    .map(|i| {
                        (
                            x(s.pre_delay_ms + s.decay_ms * i as f64 / 80.),
                            86. - 64. * s.mix * 10f64.powf(-3. * i as f64 / 80.),
                        )
                    })
                    .collect(),
            );
            out.graphs.push(graph);
            out.values.push(format!(
                "{} {}% · {} {} ms · {} {} ms",
                t("ウェット"),
                num(s.mix * 100.),
                t("プリディレイ"),
                num(s.pre_delay_ms),
                t("減衰時間"),
                num(s.decay_ms)
            ));
            out.values.push(t(
                "図は設定から描いた減衰イメージです。停止後の残響は次の開始へ持ち越しません。",
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests;
