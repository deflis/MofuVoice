use super::*;

fn preferences() -> Preferences {
    Preferences {
        language: "en".into(),
        section: "overview".into(),
    }
}
fn enabled_options() -> Options {
    let mut o = Options::default();
    o.input_eq.enabled = true;
    o.pre_conversion_eq.enabled = true;
    o.eq.enabled = true;
    o.limiter.enabled = true;
    o.gate = true;
    o.dereverb.enabled = true;
    o.deesser = true;
    o.compressor.enabled = true;
    o.level.enabled = true;
    o.level.compressor_enabled = true;
    o.reverb.enabled = true;
    o
}

#[test]
fn every_enabled_stage_has_a_diagram_and_configured_values() {
    let o = enabled_options();
    for stage in crate::audio::FILTER_STAGES {
        let preview = build(stage, &o, &preferences());
        assert!(!preview.graphs.is_empty(), "{}", stage.id());
        assert!(!preview.values.is_empty(), "{}", stage.id());
        for graph in preview.graphs {
            assert!(!graph.description.is_empty());
            for (x, y) in graph.paths.into_iter().flatten() {
                assert!(x.is_finite() && y.is_finite(), "{}", stage.id());
            }
        }
    }
    let disabled = Options {
        voice_enabled: false,
        ..Options::default()
    };
    for stage in crate::audio::FILTER_STAGES {
        assert_eq!(build(stage, &disabled, &preferences()), Preview::default());
    }
}

#[test]
fn edits_change_the_corresponding_diagram_and_summary() {
    type Edit = fn(&mut Options);
    let cases: [(FilterStage, Edit); 8] = [
        (FilterStage::NoiseGate, |o| o.gate_attack_ms = 70.),
        (FilterStage::Deesser, |o| {
            o.deesser_parameters.frequency_hz = 8000.
        }),
        (FilterStage::Voice, |o| o.pitch.semitones = 7.),
        (FilterStage::Compressor, |o| o.compressor.ratio = 8.),
        (FilterStage::Limiter, |o| o.limiter.ceiling_db = -9.),
        (FilterStage::Dereverb, |o| o.dereverb.amount = 0.9),
        (FilterStage::Level, |o| o.level.threshold_db = -24.),
        (FilterStage::Reverb, |o| o.reverb.decay_ms = 6000.),
    ];
    for (stage, edit) in cases {
        let mut o = enabled_options();
        let before = build(stage, &o, &preferences());
        edit(&mut o);
        let after = build(stage, &o, &preferences());
        assert_ne!(before.graphs, after.graphs, "{}", stage.id());
        assert_ne!(before.values, after.values, "{}", stage.id());
    }
}

#[test]
fn compressor_and_limiter_previews_match_configured_static_responses() {
    let p = preferences();
    let hard = transfer(-12., 3., false, &p, "hard");
    let soft = transfer(-12., 3., true, &p, "soft");
    let output = |point: (f64, f64)| (86. - point.1) / 64. * 60. - 60.;
    assert!((output(hard.paths[0][120]) + 8.).abs() < 1e-10);
    assert!((output(soft.paths[0][96]) + 12.5).abs() < 1e-10);
    let limiter = transfer(-3., f64::INFINITY, false, &p, "limit");
    assert!((output(limiter.paths[0][120]) + 3.).abs() < 1e-10);
}

#[test]
fn automatic_level_and_post_compressor_are_independent() {
    let mut o = enabled_options();
    o.level.compressor_enabled = false;
    let auto = build(FilterStage::Level, &o, &preferences());
    assert!(auto.graphs.is_empty());
    assert!(auto.values.iter().any(|s| s.contains("-20")));
    o.level.enabled = false;
    o.level.compressor_enabled = true;
    let compressor = build(FilterStage::Level, &o, &preferences());
    assert_eq!(compressor.graphs.len(), 1);
    assert!(!compressor.values.iter().any(|s| s.contains("-20")));
}

#[test]
fn restored_previews_have_english_labels_and_explanations() {
    let o = enabled_options();
    for stage in crate::audio::FILTER_STAGES {
        let preview = build(stage, &o, &preferences());
        let mut text = preview.values;
        for graph in preview.graphs {
            text.extend([graph.description, graph.x_label, graph.y_label]);
        }
        for value in text {
            assert!(
                !value.chars().any(|c| ('\u{3040}'..='\u{30ff}').contains(&c)
                    || ('\u{4e00}'..='\u{9fff}').contains(&c)),
                "{}: {value}",
                stage.id()
            );
        }
    }
}
