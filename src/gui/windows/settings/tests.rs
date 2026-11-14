use super::{CloseTarget, SettingsView, Widget};
use crate::{
    cli::Options,
    gui::{layout, model::AppModel, preferences::Preferences},
};
use gpui_kit::{test::TestWindowExt, AnyWindowHandle, AppContext, Entity, TestAppContext};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

fn open(
    cx: &mut TestAppContext,
    directory: &std::path::Path,
) -> (Entity<AppModel>, AnyWindowHandle, Entity<SettingsView>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::gui::theme::init(cx);
    });
    let model = cx.update(|cx| {
        cx.new(|cx| {
            AppModel::new(
                Options::default(),
                directory.join("settings.ini"),
                Preferences {
                    language: "en".into(),
                    section: "voice".into(),
                },
                cx,
            )
        })
    });
    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                layout::window_options("Settings test", 1000., 760., 800., 480.),
                cx,
                |w, cx| cx.new(|cx| SettingsView::new(model.clone(), Options::default(), w, cx)),
            )
        })
        .unwrap();
    model.update(cx, |m, _| m.settings = Some((handle, view.clone())));
    (model, handle, view)
}
fn directory() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "mofu-gpui-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[gpui_kit::test]
fn changing_eq_band_count_preserves_other_drafts_and_manual_gate_threshold(
    cx: &mut TestAppContext,
) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/noise/enabled", "true", cx);
    set(&view, window, "/noise/thresholdMode", "manual", cx);
    set(&view, window, "/noise/thresholdDb", "-35", cx);
    set(&view, window, "/noise/thresholdMode", "auto", cx);
    set(&view, window, "/voice/pitch", "7.50", cx);
    for root in ["inputEq", "preConversionEq", "eq"] {
        for add in [true, false] {
            window
                .update(cx, |_, w, cx| {
                    view.update(cx, |v, cx| v.band_count(root, add, w, cx));
                })
                .unwrap();
            cx.run_until_parked();
            assert_eq!(
                view.read_with(cx, |v, _| v.value("/noise/thresholdDb")),
                "-35"
            );
            assert_eq!(view.read_with(cx, |v, _| v.value("/voice/pitch")), "7.50");
        }
    }
    set(&view, window, "/noise/thresholdMode", "manual", cx);
    assert_eq!(
        view.read_with(cx, |v, _| v.options().unwrap().gate_threshold),
        crate::gate::GateThreshold::Db(-35.)
    );
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn settings_close_retries_after_another_app_operation_and_saves_the_draft(cx: &mut TestAppContext) {
    let dir = directory();
    std::fs::create_dir(&dir).unwrap();
    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/voice/pitch", "6.5", cx);
    model.update(cx, |m, cx| {
        m.busy = true;
        cx.notify();
    });
    view.update(cx, |v, cx| v.save(CloseTarget::Settings, cx));
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| m.settings.is_some()));
    model.update(cx, |m, cx| {
        m.busy = false;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| m.settings.is_none()));
    assert_eq!(
        crate::gui_settings::load(&dir.join("settings.ini"))
            .unwrap()
            .pitch
            .semitones,
        6.5
    );
    assert!(window.update(cx, |_, _, _| ()).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[gpui_kit::test]
fn deferred_close_keeps_the_window_and_draft_when_saving_fails(cx: &mut TestAppContext) {
    let dir = directory();
    std::fs::write(&dir, "not a directory").unwrap();
    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/voice/pitch", "6.5", cx);
    model.update(cx, |m, cx| {
        m.busy = true;
        cx.notify();
    });
    view.update(cx, |v, cx| v.save(CloseTarget::Settings, cx));
    model.update(cx, |m, cx| {
        m.busy = false;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| m.settings.is_some() && !m.busy));
    assert!(view.read_with(cx, |v, _| !v.error.is_empty()
        && v.close == CloseTarget::None
        && !v.pending));
    assert_eq!(view.read_with(cx, |v, _| v.value("/voice/pitch")), "6.5");
    cleanup(model, window, cx);
    std::fs::remove_file(dir).unwrap();
}

#[gpui_kit::test]
fn input_survives_redraw_and_navigation_and_saves_to_shared_model(cx: &mut TestAppContext) {
    let directory = directory();
    std::fs::create_dir(&directory).unwrap();
    let (model, window, view) = open(cx, &directory);
    let input = view.read_with(cx, |v, _| {
        let Widget::Input(input) = &v
            .controls
            .iter()
            .find(|c| c.field.path == "/voice/pitch")
            .unwrap()
            .widget
        else {
            panic!("pitch input")
        };
        input.clone()
    });
    window
        .update(cx, |_, w, cx| {
            input.update(cx, |s, cx| s.focus(w, cx));
            w.render_frame(cx);
            w.press(
                if cfg!(target_os = "macos") {
                    "cmd-a"
                } else {
                    "ctrl-a"
                },
                cx,
            );
            w.input("7.5", cx);
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |v, _| v.value("/voice/pitch")), "7.5");
    model.update(cx, |m, cx| {
        m.preferences.section = "devices".into();
        cx.notify();
    });
    cx.run_until_parked();
    model.update(cx, |m, cx| {
        m.preferences.section = "voice".into();
        cx.notify();
    });
    cx.run_until_parked();
    window.update(cx, |_, w, cx| w.render_frame(cx)).unwrap();
    assert_eq!(
        view.read_with(cx, |v, _| v.options().unwrap().pitch.semitones),
        7.5
    );
    view.update(cx, |v, cx| v.save(CloseTarget::None, cx));
    cx.run_until_parked();
    assert_eq!(model.read_with(cx, |m, _| m.saved.pitch.semitones), 7.5);
    assert_eq!(
        crate::gui_settings::load(&directory.join("settings.ini"))
            .unwrap()
            .pitch
            .semitones,
        7.5
    );
    model.update(cx, |m, _| m.settings = None);
    window.update(cx, |_, w, _| w.remove_window()).unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}

#[gpui_kit::test]
fn failed_save_keeps_the_draft_and_cancels_application_close(cx: &mut TestAppContext) {
    let directory = directory();
    std::fs::write(&directory, "parent is a file").unwrap(); // A parent file prevents create_dir_all.
    let (model, window, view) = open(cx, &directory);
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| v.set_value("/voice/pitch", "8".into(), w, cx))
        })
        .unwrap();
    model.update(cx, |m, _| m.quit_requested = true);
    view.update(cx, |v, cx| {
        v.save(CloseTarget::Application, cx);
        v.save(CloseTarget::Application, cx);
    });
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| !v.error.is_empty() && !v.pending));
    assert_eq!(view.read_with(cx, |v, _| v.value("/voice/pitch")), "8");
    assert!(model.read_with(cx, |m, _| !m.closing
        && !m.quit_requested
        && m.saved.pitch.semitones == 0.));
    model.update(cx, |m, _| m.settings = None);
    window.update(cx, |_, w, _| w.remove_window()).unwrap();
    std::fs::remove_file(directory).unwrap();
}

#[gpui_kit::test]
fn settings_window_is_reused_and_can_be_closed_and_reopened(cx: &mut TestAppContext) {
    let directory = directory();
    std::fs::create_dir(&directory).unwrap();
    let (model, first, view) = open(cx, &directory);
    model.update(cx, |m, cx| {
        m.busy = true;
        m.open_settings(Some("inputEq".into()), cx);
    });
    assert_eq!(
        model.read_with(cx, |m, _| m.settings.as_ref().unwrap().0),
        first
    );
    assert_eq!(
        model.read_with(cx, |m, _| m.preferences.section.clone()),
        "inputEq"
    );
    model.update(cx, |m, _| m.busy = false);
    view.update(cx, |v, cx| v.save(CloseTarget::Settings, cx));
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| m.settings.is_none()));
    model.update(cx, |m, cx| {
        m.busy = true;
        m.open_settings(Some("voice".into()), cx);
    });
    cx.run_until_parked();
    let second = model.read_with(cx, |m, _| m.settings.as_ref().unwrap().0);
    assert_ne!(first, second);
    model.update(cx, |m, _| m.settings = None);
    second.update(cx, |_, w, _| w.remove_window()).unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}

fn navigate(
    model: &Entity<AppModel>,
    window: AnyWindowHandle,
    section: &str,
    cx: &mut TestAppContext,
) {
    model.update(cx, |m, cx| {
        m.preferences.section = section.into();
        cx.notify();
    });
    cx.run_until_parked();
    window.update(cx, |_, w, cx| w.render_frame(cx)).unwrap();
}
fn set(
    view: &Entity<SettingsView>,
    window: AnyWindowHandle,
    path: &str,
    text: &str,
    cx: &mut TestAppContext,
) {
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.set_value(path, text.into(), w, cx);
                w.refresh();
                cx.notify();
            })
        })
        .unwrap();
    cx.run_until_parked();
}
fn cleanup(model: Entity<AppModel>, window: AnyWindowHandle, cx: &mut TestAppContext) {
    model.update(cx, |m, _| m.settings = None);
    window.update(cx, |_, w, _| w.remove_window()).unwrap();
}
fn drag_slider(window: AnyWindowHandle, path: &str, fraction: f32, cx: &mut TestAppContext) {
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            let bounds = w
                .find(gpui_kit::SharedString::from(format!("slider:{path}")))
                .bounds();
            let center = bounds.center();
            let end = gpui_kit::point(bounds.left() + bounds.size.width * fraction, center.y);
            w.drag(center, end, cx);
        })
        .unwrap();
    cx.run_until_parked();
}
#[gpui_kit::test]
fn eq_simple_slider_and_details_share_values_and_reset_preserves_shape(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    for (section, root) in [
        ("inputEq", "inputEq"),
        ("preConversionEq", "preConversionEq"),
        ("postConversionEq", "eq"),
    ] {
        set(&view, window, &format!("/{root}/enabled"), "true", cx);
        navigate(&model, window, section, cx);
        let gain = format!("/{root}/bands/0/gainDb");
        let frequency = format!("/{root}/bands/0/frequency");
        let q = format!("/{root}/bands/0/q");
        set(&view, window, &frequency, "380", cx);
        set(&view, window, &q, "1.7", cx);
        drag_slider(window, &gain, 0.75, cx);
        let changed = view.read_with(cx, |v, _| v.value(&gain).parse::<f64>().unwrap());
        assert!((4.5..=7.5).contains(&changed), "slider value {changed}");
        window
            .update(cx, |_, w, cx| {
                w.click(
                    gpui_kit::SharedString::from(format!("details:{section}")),
                    cx,
                )
            })
            .unwrap();
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |v, _| v.value(&format!("/{root}/mode"))),
            "pro"
        );
        assert_eq!(view.read_with(cx, |v, _| v.value(&frequency)), "380");
        set(&view, window, &gain, "-3.25", cx);
        window
            .update(cx, |_, w, cx| w.click("close-details", cx))
            .unwrap();
        cx.run_until_parked();
        assert_eq!(view.read_with(cx, |v, _| v.value(&gain)), "-3.25");
        window
            .update(cx, |_, w, cx| {
                w.click(
                    gpui_kit::SharedString::from(format!("details:{section}")),
                    cx,
                )
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |_, w, cx| {
                w.scroll(
                    "details-content",
                    gpui_kit::ScrollDelta::Pixels(gpui_kit::point(
                        gpui_kit::px(0.),
                        gpui_kit::px(-1200.),
                    )),
                    cx,
                );
                w.click(gpui_kit::SharedString::from(format!("reset:{section}")), cx)
            })
            .unwrap();
        cx.run_until_parked();
        assert_eq!(view.read_with(cx, |v, _| v.value(&gain)), "0");
        assert_eq!(view.read_with(cx, |v, _| v.value(&frequency)), "380");
        assert_eq!(view.read_with(cx, |v, _| v.value(&q)), "1.7");
        window.update(cx, |_, w, cx| w.press("escape", cx)).unwrap();
        cx.run_until_parked();
    }
    cleanup(model, window, cx);
}
#[gpui_kit::test]
fn deesser_simple_amount_preserves_custom_details_and_reset_preserves_mode(
    cx: &mut TestAppContext,
) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/deesser/enabled", "true", cx);
    set(&view, window, "/deesser/frequencyHz", "6500", cx);
    set(&view, window, "/deesser/thresholdDb", "-30", cx);
    navigate(&model, window, "deesser", cx);
    drag_slider(window, "/deesser/rangeDb", 0.75, cx);
    assert_eq!(
        view.read_with(cx, |v, _| v.value("/deesser/frequencyHz")),
        "6500"
    );
    assert_eq!(
        view.read_with(cx, |v, _| v.value("/deesser/thresholdDb")),
        "-30"
    );
    window
        .update(cx, |_, w, cx| w.click("details:deesser", cx))
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |_, w, cx| {
            w.scroll(
                "details-content",
                gpui_kit::ScrollDelta::Pixels(gpui_kit::point(
                    gpui_kit::px(0.),
                    gpui_kit::px(-1500.),
                )),
                cx,
            );
            w.click("reset:deesser", cx)
        })
        .unwrap();
    cx.run_until_parked();
    for (path, expected) in [
        ("/deesser/rangeDb", 6.),
        ("/deesser/frequencyHz", 5000.),
        ("/deesser/thresholdDb", -24.),
        ("/deesser/attackMs", 0.5),
        ("/deesser/releaseMs", 60.),
    ] {
        assert_eq!(
            view.read_with(cx, |v, _| v.value(path).parse::<f64>().unwrap()),
            expected
        );
    }
    assert_eq!(
        view.read_with(cx, |v, _| v.value("/deesser/enabled")),
        "true"
    );
    assert_eq!(view.read_with(cx, |v, _| v.value("/deesser/mode")), "pro");
    cleanup(model, window, cx);
}
#[gpui_kit::test]
fn pitch_percent_does_not_change_formant_units_or_discard_an_unfinished_other_field(
    cx: &mut TestAppContext,
) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/voice/pitch", "12", cx);
    set(&view, window, "/voice/formantShift", "3", cx);
    set(&view, window, "/compressor/ratio", "-", cx);
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.changed("/voice/pitchUnit", "percent".into(), w, cx)
            })
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |v, _| v.value("/voice/pitch").parse::<f64>().unwrap()),
        200.
    );
    assert_eq!(
        view.read_with(cx, |v, _| v.value("/voice/formantShift")),
        "3"
    );
    assert_eq!(view.read_with(cx, |v, _| v.value("/compressor/ratio")), "-");
    let range = view.read_with(cx, |v, cx| {
        let slider = v
            .controls
            .iter()
            .find(|c| c.field.path == "/voice/formantShift")
            .unwrap()
            .slider
            .as_ref()
            .unwrap()
            .read(cx);
        (slider.min_value(), slider.max_value())
    });
    assert_eq!(range, (-12., 12.));
    window
        .update(cx, |_, w, cx| w.click("reset:voice", cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |v, _| v.value("/voice/pitch")), "100");
    assert_eq!(
        view.read_with(cx, |v, _| v.value("/voice/formantShift")),
        "0"
    );
    cleanup(model, window, cx);
}
#[gpui_kit::test]
fn voice_dependencies_and_hidden_manual_threshold_match_react(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.changed("/voice/preserveFormants", "false".into(), w, cx);
                v.changed("/voice/pitchAlgorithm", "td-psola".into(), w, cx);
            })
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |v, _| v.value("/voice/preserveFormants")),
        "true"
    );
    assert!(!view.read_with(cx, |v, cx| v.editable("/voice/preserveFormants", cx)));
    set(&view, window, "/noise/enabled", "true", cx);
    navigate(&model, window, "noiseGate", cx);
    window
        .update(cx, |_, w, cx| w.click("details:noiseGate", cx))
        .unwrap();
    cx.run_until_parked();
    assert!(!view.read_with(cx, |v, cx| v.editable("/noise/thresholdDb", cx)));
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.changed("/noise/thresholdMode", "manual".into(), w, cx)
            })
        })
        .unwrap();
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, cx| v.editable("/noise/thresholdDb", cx)));
    cleanup(model, window, cx);
}
#[gpui_kit::test]
fn invalid_numeric_draft_disables_simple_slider_and_audio_locks_details(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/deesser/enabled", "true", cx);
    navigate(&model, window, "deesser", cx);
    set(&view, window, "/deesser/rangeDb", "-", cx);
    drag_slider(window, "/deesser/rangeDb", 0.9, cx);
    assert_eq!(view.read_with(cx, |v, _| v.value("/deesser/rangeDb")), "-");
    window
        .update(cx, |_, w, cx| w.click("details:deesser", cx))
        .unwrap();
    cx.run_until_parked();
    model.update(cx, |m, cx| {
        m.active = true;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(!view.read_with(cx, |v, cx| v.editable("/deesser/rangeDb", cx)));
    window
        .update(cx, |_, w, cx| {
            w.scroll(
                "details-content",
                gpui_kit::ScrollDelta::Pixels(gpui_kit::point(
                    gpui_kit::px(0.),
                    gpui_kit::px(-1500.),
                )),
                cx,
            );
            w.click("reset:deesser", cx)
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |v, _| v.value("/deesser/rangeDb")), "-");
    model.update(cx, |m, _| m.active = false);
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn voice_sliders_and_numeric_arrows_stay_synchronized(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    drag_slider(window, "/voice/pitch", 0.75, cx);
    let pitch = view.read_with(cx, |v, _| v.value("/voice/pitch").parse::<f64>().unwrap());
    assert!((4.5..=7.5).contains(&pitch));
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.changed("/voice/pitchUnit", "percent".into(), w, cx)
            })
        })
        .unwrap();
    cx.run_until_parked();
    drag_slider(window, "/voice/formantShift", 0.75, cx);
    let formant = view.read_with(cx, |v, _| {
        v.value("/voice/formantShift").parse::<f64>().unwrap()
    });
    assert!((4.5..=7.5).contains(&formant));
    let input = view.read_with(cx, |v, _| {
        let Widget::Input(input) = &v
            .controls
            .iter()
            .find(|c| c.field.path == "/voice/formantShift")
            .unwrap()
            .widget
        else {
            panic!()
        };
        input.clone()
    });
    window
        .update(cx, |_, w, cx| {
            input.update(cx, |s, cx| s.focus(w, cx));
            w.render_frame(cx);
            w.press("up", cx);
        })
        .unwrap();
    cx.run_until_parked();
    let number = view.read_with(cx, |v, _| {
        v.value("/voice/formantShift").parse::<f64>().unwrap()
    });
    assert!((number - formant - 0.01).abs() < 1e-8);
    let slider = view.read_with(cx, |v, cx| {
        v.controls
            .iter()
            .find(|c| c.field.path == "/voice/formantShift")
            .unwrap()
            .slider
            .as_ref()
            .unwrap()
            .read(cx)
            .value()
    });
    let gpui_kit::component::slider::SliderValue::Single(value) = slider else {
        panic!()
    };
    assert!((value as f64 - number).abs() < 1e-5);
    cleanup(model, window, cx);
}
#[gpui_kit::test]
fn level_resets_are_independent_and_window_choices_follow_analysis(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/level/enabled", "true", cx);
    set(&view, window, "/level/compressorEnabled", "true", cx);
    set(&view, window, "/level/targetDb", "-16", cx);
    set(&view, window, "/level/thresholdDb", "-22", cx);
    navigate(&model, window, "autoLevel", cx);
    window
        .update(cx, |_, w, cx| w.click("details:autoLevel", cx))
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |_, w, cx| w.click("reset:autoLevel", cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |v, _| v
            .value("/level/targetDb")
            .parse::<f64>()
            .unwrap()),
        -20.
    );
    assert_eq!(
        view.read_with(cx, |v, _| v.value("/level/thresholdDb")),
        "-22"
    );
    window
        .update(cx, |_, w, cx| w.click("close-details", cx))
        .unwrap();
    cx.run_until_parked();
    navigate(&model, window, "postCompressor", cx);
    window
        .update(cx, |_, w, cx| w.click("details:postCompressor", cx))
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |_, w, cx| w.click("reset:postCompressor", cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |v, _| v
            .value("/level/thresholdDb")
            .parse::<f64>()
            .unwrap()),
        -12.
    );
    assert_eq!(view.read_with(cx, |v, _| v.value("/level/enabled")), "true");
    assert_eq!(
        view.read_with(cx, |v, _| v.value("/level/compressorEnabled")),
        "true"
    );
    window
        .update(cx, |_, w, cx| w.click("close-details", cx))
        .unwrap();
    cx.run_until_parked();
    set(&view, window, "/voice/synthesisWindow", "2048", cx);
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.changed("/voice/analysisWindow", "1024".into(), w, cx)
            })
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |v, _| v.value("/voice/synthesisWindow")),
        "1024"
    );
    navigate(&model, window, "voice", cx);
    window
        .update(cx, |_, w, cx| w.click("details:voice", cx))
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |_, w, _cx| {
            assert!(w.find("field:/voice/analysisWindow").visible());
            assert!(w.try_find("field:/voice/psolaAnalysisWindow").is_none());
        })
        .unwrap();
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn detailed_values_survive_disk_save_and_reopen_in_the_simple_page(cx: &mut TestAppContext) {
    let dir = directory();
    std::fs::create_dir(&dir).unwrap();
    let (model, window, view) = open(cx, &dir);
    for (path, text) in [
        ("/inputEq/enabled", "true"),
        ("/inputEq/bands/0/frequency", "380"),
        ("/inputEq/bands/0/gainDb", "-2.5"),
        ("/inputEq/bands/0/q", "1.7"),
        ("/deesser/enabled", "true"),
        ("/deesser/frequencyHz", "6500"),
        ("/deesser/rangeDb", "9.5"),
    ] {
        set(&view, window, path, text, cx);
    }
    navigate(&model, window, "inputEq", cx);
    window
        .update(cx, |_, w, cx| w.click("details:inputEq", cx))
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |_, w, cx| w.click("close-details", cx))
        .unwrap();
    cx.run_until_parked();
    view.update(cx, |v, cx| v.save(CloseTarget::Settings, cx));
    cx.run_until_parked();
    let saved = crate::gui_settings::load(&dir.join("settings.ini")).unwrap();
    assert_eq!(saved.input_eq_mode, "pro");
    assert_eq!(saved.input_eq.bands[0].frequency, 380.);
    assert_eq!(saved.input_eq.bands[0].gain_db, -2.5);
    assert_eq!(saved.deesser_parameters.frequency_hz, 6500.);
    model.update(cx, |m, cx| {
        m.busy = true;
        m.open_settings(Some("inputEq".into()), cx);
    });
    cx.run_until_parked();
    let (reopened, editor) = model.read_with(cx, |m, _| m.settings.as_ref().unwrap().clone());
    assert_eq!(
        editor.read_with(cx, |v, _| v
            .value("/inputEq/bands/0/q")
            .parse::<f64>()
            .unwrap()),
        1.7
    );
    reopened
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            assert!(w.find("slider:/inputEq/bands/0/gainDb").visible());
            assert!(w.try_find("field:/inputEq/bands/0/frequency").is_none());
        })
        .unwrap();
    cleanup(model, reopened, cx);
    std::fs::remove_dir_all(dir).unwrap();
}

#[gpui_kit::test]
fn changing_theme_preserves_modal_input_and_updates_existing_and_new_windows(
    cx: &mut TestAppContext,
) {
    use gpui_kit::WindowAppearance;
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/deesser/enabled", "true", cx);
    set(&view, window, "/deesser/frequencyHz", "6500", cx);
    navigate(&model, window, "deesser", cx);
    window
        .update(cx, |_, w, cx| w.click("details:deesser", cx))
        .unwrap();
    cx.run_until_parked();
    for appearance in [
        WindowAppearance::Dark,
        WindowAppearance::Light,
        WindowAppearance::Dark,
    ] {
        cx.update(|cx| crate::gui::theme::apply_appearance(appearance, cx));
        cx.run_until_parked();
        window.update(cx, |_, w, cx| w.render_frame(cx)).unwrap();
        assert_eq!(
            view.read_with(cx, |v, _| v.value("/deesser/frequencyHz")),
            "6500"
        );
        window
            .update(cx, |_, w, cx| {
                assert!(w.find("field:/deesser/frequencyHz").visible());
                let p = crate::gui::theme::palette(cx);
                assert_eq!(gpui_kit::component::Theme::global(cx).popover, p.surface);
            })
            .unwrap();
    }
    model.update(cx, |m, cx| m.open_log(cx));
    cx.run_until_parked();
    let log = model.read_with(cx, |m, _| m.log_window.unwrap());
    log.update(cx, |_, w, cx| {
        w.render_frame(cx);
        assert!(gpui_kit::component::Theme::global(cx).is_dark());
    })
    .unwrap();
    window
        .update(cx, |_, w, cx| w.click("close-details", cx))
        .unwrap();
    cx.run_until_parked();
    model.update(cx, |m, _| m.log_window = None);
    log.update(cx, |_, w, _| w.remove_window()).unwrap();
    cleanup(model, window, cx);
}
