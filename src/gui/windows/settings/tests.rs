use super::{CloseTarget, Operation, SettingsView, Widget};
use crate::{
    cli::Options,
    gui::{layout, model::AppModel, preferences::Preferences},
};
use gpui_kit::{test::TestWindowExt, AnyWindowHandle, AppContext, Entity, TestAppContext};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

pub(super) fn open(
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
pub(super) fn directory() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "mofu-gpui-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[gpui_kit::test]
fn failed_device_discovery_replaces_pending_status(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    model.update(cx, |m, cx| {
        m.preferences.section = "devices".into();
        cx.notify();
    });
    view.update(cx, |v, cx| {
        v.operation = Some(Operation::Save);
        cx.notify();
    });
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("device-discovery-pending").is_none());
        })
        .unwrap();
    view.update(cx, |v, cx| {
        v.operation = Some(Operation::Discover);
        cx.notify();
    });
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("device-discovery-pending").is_some());
        })
        .unwrap();
    view.update(cx, |v, cx| {
        v.operation = None;
        v.device_error = "device disconnected".into();
        cx.notify();
    });
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("device-discovery-pending").is_none());
            assert!(w.try_find("device-discovery-failed").is_some());
        })
        .unwrap();
    cleanup(model, window, cx);
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
fn changing_eq_band_count_preserves_discovered_device_choices(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    let devices = [
        ("/devices/device", ["", "Microphone A", "Microphone B"]),
        ("/devices/outputDevice", ["", "Output A", "Output B"]),
        ("/devices/previewDevice", ["", "Output A", "Output B"]),
    ];
    view.update(cx, |v, _| {
        for (path, items) in devices {
            let control = v
                .controls
                .iter_mut()
                .find(|c| c.field.path == path)
                .unwrap();
            control.items = Some(items.map(str::to_owned).into());
        }
    });
    for add in [true, false] {
        window
            .update(cx, |_, window, cx| {
                view.update(cx, |v, cx| v.band_count("eq", add, window, cx));
            })
            .unwrap();
        view.read_with(cx, |v, _| {
            for (path, items) in devices {
                let control = v.controls.iter().find(|c| c.field.path == path).unwrap();
                assert_eq!(control.items.as_deref().unwrap(), items);
            }
        });
    }
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
            .options
            .pitch
            .semitones,
        6.5
    );
    assert!(window.update(cx, |_, _, _| ()).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[gpui_kit::test]
fn settings_close_persists_ui_preferences_with_audio_options(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/voice/pitch", "6.5", cx);
    model.update(cx, |m, cx| {
        m.preferences.language = "ja".into();
        m.preferences.section = "devices".into();
        cx.notify();
    });

    view.update(cx, |v, cx| v.save(CloseTarget::Settings, cx));
    cx.run_until_parked();

    let saved = crate::gui_settings::load(&dir.join("settings.ini")).unwrap();
    assert_eq!(saved.options.pitch.semitones, 6.5);
    assert_eq!(saved.ui.language.as_deref(), Some("ja"));
    assert_eq!(saved.ui.section.as_deref(), Some("devices"));
    assert!(model.read_with(cx, |m, _| m.settings.is_none()));
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
        && !v.pending()));
    assert_eq!(view.read_with(cx, |v, _| v.value("/voice/pitch")), "6.5");
    cleanup(model, window, cx);
    std::fs::remove_file(dir).unwrap();
}

#[gpui_kit::test]
fn overview_renders_active_stages_and_isolates_unfinished_fields(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    for path in [
        "/inputEq/enabled",
        "/preConversionEq/enabled",
        "/eq/enabled",
        "/limiter/enabled",
        "/noise/enabled",
        "/dereverb/enabled",
        "/deesser/enabled",
        "/compressor/enabled",
        "/level/enabled",
        "/level/compressorEnabled",
        "/reverb/enabled",
    ] {
        set(&view, window, path, "true", cx);
    }
    navigate(&model, window, "overview", cx);
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            for stage in crate::audio::FILTER_STAGES {
                assert!(
                    w.try_find(gpui_kit::SharedString::from(format!(
                        "overview-plot:{}:0",
                        stage.id()
                    )))
                    .is_some(),
                    "{}",
                    stage.id()
                );
            }
        })
        .unwrap();
    set(&view, window, "/compressor/ratio", "-", cx);
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("overview-plot:compressor:0").is_none());
            assert!(w.try_find("overview-plot:voice:0").is_some());
        })
        .unwrap();
    set(&view, window, "/compressor/ratio", "3", cx);
    set(&view, window, "/inputEq/enabled", "false", cx);
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("overview-plot:inputEq:0").is_none());
        })
        .unwrap();
    set(&view, window, "/noise/rumbleFilter", "true", cx);
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("overview-plot:inputEq:0").is_some());
        })
        .unwrap();
    cleanup(model, window, cx);
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
            .options
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
    assert!(view.read_with(cx, |v, _| !v.error.is_empty() && !v.pending()));
    assert_eq!(view.read_with(cx, |v, _| v.value("/voice/pitch")), "8");
    assert!(model.read_with(cx, |m, _| !m.closing
        && !m.quit_requested
        && m.saved.pitch.semitones == 0.));
    model.update(cx, |m, _| m.settings = None);
    window.update(cx, |_, w, _| w.remove_window()).unwrap();
    std::fs::remove_file(directory).unwrap();
}

#[gpui_kit::test]
fn application_close_activates_settings_when_draft_is_invalid(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, settings, view) = open(cx, &dir);
    let (main, _) = cx
        .update(|cx| {
            gpui_kit::open_window(
                crate::gui::windows::main::window_options(),
                cx,
                |window, cx| {
                    cx.new(|cx| crate::gui::windows::main::MainView::new(model.clone(), window, cx))
                },
            )
        })
        .unwrap();
    set(&view, settings, "/voice/pitch", "", cx);
    main.update(cx, |_, window, _| window.activate_window())
        .unwrap();
    assert_eq!(cx.update(|cx| cx.active_window()), Some(main));

    model.update(cx, |m, cx| m.request_quit(cx));
    cx.run_until_parked();

    assert_eq!(cx.update(|cx| cx.active_window()), Some(settings));
    assert!(view.read_with(cx, |v, _| !v.error.is_empty()));
    assert!(model.read_with(cx, |m, _| !m.quit_requested && !m.closing));
    cleanup(model, settings, cx);
    main.update(cx, |_, window, _| window.remove_window())
        .unwrap();
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
pub(super) fn set(
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
pub(super) fn cleanup(model: Entity<AppModel>, window: AnyWindowHandle, cx: &mut TestAppContext) {
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
    let saved = crate::gui_settings::load(&dir.join("settings.ini"))
        .unwrap()
        .options;
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

#[gpui_kit::test]
fn pitch_unit_switch_rejects_out_of_range_semitones(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    for invalid in ["20", "-13", "12000"] {
        set(&view, window, "/voice/pitch", invalid, cx);
        window
            .update(cx, |_, w, cx| {
                view.update(cx, |v, cx| {
                    v.changed("/voice/pitchUnit", "percent".into(), w, cx)
                })
            })
            .unwrap();
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |v, _| v.value("/voice/pitchUnit")),
            "semitones"
        );
        assert_eq!(view.read_with(cx, |v, _| v.value("/voice/pitch")), invalid);
        assert!(view.read_with(cx, |v, _| !v.error.is_empty()));
    }
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn unmodified_settings_close_skips_disk_write_and_failed_save_closes_on_retry(
    cx: &mut TestAppContext,
) {
    let dir = directory();
    std::fs::write(&dir, "not a directory").unwrap();

    let (model, window, view) = open(cx, &dir);
    view.update(cx, |v, cx| v.save(CloseTarget::Settings, cx));
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| m.settings.is_none()));
    assert!(window.update(cx, |_, _, _| ()).is_err());

    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/voice/pitch", "6.5", cx);
    view.update(cx, |v, cx| v.save(CloseTarget::Settings, cx));
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| m.settings.is_some()));
    assert!(view.read_with(cx, |v, _| v.save_failed && !v.error.is_empty()));

    view.update(cx, |v, cx| v.save(CloseTarget::Settings, cx));
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| m.settings.is_none()));
    assert!(window.update(cx, |_, _, _| ()).is_err());

    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/voice/pitch", "7", cx);
    view.update(cx, |v, cx| v.save(CloseTarget::Application, cx));
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| !m.closing));
    assert!(view.read_with(cx, |v, _| v.save_failed));

    view.update(cx, |v, cx| v.save(CloseTarget::Application, cx));
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| m.closing));
    cleanup(model, window, cx);
    std::fs::remove_file(dir).unwrap();
}

#[gpui_kit::test]
fn pitch_unit_and_eq_band_edits_clear_saved_notice(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    view.update(cx, |v, _| v.notice = "設定を保存しました。".into());
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.changed("/voice/pitchUnit", "percent".into(), w, cx)
            })
        })
        .unwrap();
    assert!(view.read_with(cx, |v, _| v.notice.is_empty()));

    view.update(cx, |v, _| v.notice = "設定を保存しました。".into());
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| v.band_count("eq", true, w, cx))
        })
        .unwrap();
    assert!(view.read_with(cx, |v, _| v.notice.is_empty()));
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn missing_device_and_unavailable_input_channel_are_labeled(cx: &mut TestAppContext) {
    use super::fields::labeled_choices;
    use crate::backend::{BackendKind, DeviceChannels, DeviceSampleRates};
    use gpui_kit::SharedString;

    let en = Preferences {
        language: "en".into(),
        section: "devices".into(),
    };
    let ja = Preferences {
        language: "ja".into(),
        section: "devices".into(),
    };
    let en_choices = labeled_choices(
        "/devices/device",
        vec![String::new(), "OldMic".into()],
        &en,
        BackendKind::Cpal,
        Some("OldMic"),
    );
    assert_eq!(
        en_choices[1].label,
        SharedString::from("OldMic (not detected)")
    );
    let ja_choices = labeled_choices(
        "/devices/device",
        vec![String::new(), "OldMic".into()],
        &ja,
        BackendKind::Cpal,
        Some("OldMic"),
    );
    assert_eq!(ja_choices[1].label, SharedString::from("OldMic（未検出）"));

    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/devices/inputChannel", "4", cx);
    view.update(cx, |v, cx| {
        v.rates = Some(DeviceSampleRates {
            channels: vec![DeviceChannels {
                sample_rate: 48000,
                input: 2,
                output: 2,
            }],
            input: vec![48000],
            output: vec![48000],
            common: vec![48000],
        });
        cx.notify();
    });
    navigate(&model, window, "devices", cx);
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("input-channel:4").is_some());
        })
        .unwrap();
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn initial_device_choices_do_not_duplicate_empty_default_and_backend_switch_clears_stale_items(
    cx: &mut TestAppContext,
) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    view.read_with(cx, |v, _| {
        for path in [
            "/devices/device",
            "/devices/outputDevice",
            "/devices/previewDevice",
        ] {
            let control = v.controls.iter().find(|c| c.field.path == path).unwrap();
            assert_eq!(control.items.as_deref(), Some(&[String::new()][..]));
        }
    });
    view.update(cx, |v, _| {
        for (path, items) in [
            ("/devices/device", vec![String::new(), "Old Mic".into()]),
            (
                "/devices/outputDevice",
                vec![String::new(), "Old Speaker".into()],
            ),
            (
                "/devices/previewDevice",
                vec![String::new(), "Old Headphones".into()],
            ),
        ] {
            let control = v
                .controls
                .iter_mut()
                .find(|c| c.field.path == path)
                .unwrap();
            control.items = Some(items);
            control.missing = Some("Old Missing".into());
        }
    });
    set(&view, window, "/devices/device", "Old Mic", cx);
    let current_backend = view.read_with(cx, |v, _| v.value("/devices/backend"));
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.changed("/devices/backend", current_backend, w, cx);
            });
        })
        .unwrap();
    view.read_with(cx, |v, _| {
        assert!(!v.pending());
        assert_eq!(v.value("/devices/device"), "Old Mic");
        let control = v
            .controls
            .iter()
            .find(|c| c.field.path == "/devices/device")
            .unwrap();
        assert_eq!(
            control.items.as_deref(),
            Some(&[String::new(), "Old Mic".into()][..])
        );
    });
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.reset_device_choices(w, cx);
                v.apply_discovery(Err("discovery failed".into()), w, cx);
            });
        })
        .unwrap();
    view.read_with(cx, |v, _| {
        assert_eq!(v.device_error, "discovery failed");
        for path in [
            "/devices/device",
            "/devices/outputDevice",
            "/devices/previewDevice",
        ] {
            let control = v.controls.iter().find(|c| c.field.path == path).unwrap();
            assert_eq!(control.items.as_deref(), Some(&[String::new()][..]));
            assert_eq!(control.missing, None);
            assert_eq!(control.field.text, "");
        }
    });
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn asio_discovery_ignores_unused_cpal_output_and_preview_devices(cx: &mut TestAppContext) {
    use crate::{backend::BackendKind, gui::model::DeviceChoices};

    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/devices/device", "ASIO Driver", cx);
    set(&view, window, "/devices/outputDevice", "Old CPAL Out", cx);
    set(
        &view,
        window,
        "/devices/previewDevice",
        "Old CPAL Preview",
        cx,
    );
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.apply_discovery(
                    Ok((
                        DeviceChoices {
                            inputs: vec!["ASIO Driver".into()],
                            outputs: Vec::new(),
                            resolved: BackendKind::Asio,
                        },
                        Ok(None),
                    )),
                    w,
                    cx,
                );
            });
        })
        .unwrap();
    view.read_with(cx, |v, _| {
        assert!(v.device_error.is_empty());
        for path in [
            "/devices/device",
            "/devices/outputDevice",
            "/devices/previewDevice",
        ] {
            let control = v.controls.iter().find(|c| c.field.path == path).unwrap();
            assert_eq!(control.missing, None);
        }
    });
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn missing_device_message_is_not_overwritten_by_rate_lookup_failure(cx: &mut TestAppContext) {
    use crate::{backend::BackendKind, gui::model::DeviceChoices};

    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/devices/device", "MissingMic", cx);
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.apply_discovery(
                    Ok((
                        DeviceChoices {
                            inputs: vec!["PresentMic".into()],
                            outputs: vec!["PresentOut".into()],
                            resolved: BackendKind::Cpal,
                        },
                        Err("raw backend error".into()),
                    )),
                    w,
                    cx,
                );
            });
        })
        .unwrap();
    view.read_with(cx, |v, _| {
        assert_eq!(
            v.device_error,
            "前回使った音声機器が見つかりません。入力・出力デバイスを確認してください。"
        );
    });
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn closing_disables_settings_navigation_and_overview_stage_buttons(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    navigate(&model, window, "overview", cx);
    view.update(cx, |v, _| v.save_failed = true);
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            w.click("stage-inputEq", cx);
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        model.read_with(cx, |m, _| m.preferences.section.clone()),
        "inputEq"
    );
    assert!(view.read_with(cx, |v, _| !v.save_failed));

    view.update(cx, |v, _| v.save_failed = true);
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            w.click("nav-overview", cx);
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        model.read_with(cx, |m, _| m.preferences.section.clone()),
        "overview"
    );
    assert!(view.read_with(cx, |v, _| !v.save_failed));

    model.update(cx, |m, cx| {
        m.quit_requested = true;
        cx.notify();
    });
    cx.run_until_parked();
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            w.click("nav-devices", cx);
            w.click("stage-inputEq", cx);
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        model.read_with(cx, |m, _| m.preferences.section.clone()),
        "overview"
    );

    model.update(cx, |m, cx| {
        m.quit_requested = false;
        m.closing = true;
        cx.notify();
    });
    cx.run_until_parked();
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            w.click("nav-devices", cx);
            w.click("stage-inputEq", cx);
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        model.read_with(cx, |m, _| m.preferences.section.clone()),
        "overview"
    );
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn out_of_range_input_and_output_channels_still_offer_valid_device_channels(
    cx: &mut TestAppContext,
) {
    use crate::backend::{DeviceChannels, DeviceSampleRates};

    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/devices/inputChannel", "4", cx);
    set(&view, window, "/devices/outputChannels", "3,4", cx);
    view.update(cx, |v, cx| {
        v.rates = Some(DeviceSampleRates {
            channels: vec![DeviceChannels {
                sample_rate: 48000,
                input: 2,
                output: 2,
            }],
            input: vec![48000],
            output: vec![48000],
            common: vec![48000],
        });
        cx.notify();
    });
    navigate(&model, window, "devices", cx);
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            for id in [
                "input-channel:1",
                "input-channel:2",
                "input-channel:4",
                "output-channel:1",
                "output-channel:2",
                "output-channel:3",
                "output-channel:4",
            ] {
                assert!(w.try_find(id).is_some(), "{id}");
            }
            w.click("input-channel:1", cx);
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |v, _| v.value("/devices/inputChannel")),
        "1"
    );
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn asio_unselected_shows_rate_hint_and_preview_device_change_skips_rediscovery(
    cx: &mut TestAppContext,
) {
    use crate::backend::{BackendKind, DeviceChannels, DeviceSampleRates};

    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/devices/backend", "asio", cx);
    set(&view, window, "/devices/device", "", cx);
    navigate(&model, window, "devices", cx);
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("device-select-hint").is_some());
        })
        .unwrap();

    set(
        &view,
        window,
        "/devices/backend",
        BackendKind::Cpal.name(),
        cx,
    );
    view.update(cx, |v, cx| {
        v.rates = Some(DeviceSampleRates {
            channels: vec![DeviceChannels {
                sample_rate: 48000,
                input: 2,
                output: 2,
            }],
            input: vec![48000],
            output: vec![48000],
            common: vec![48000],
        });
        cx.notify();
    });
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.changed("/devices/previewDevice", "Headphones".into(), w, cx);
            });
        })
        .unwrap();
    view.read_with(cx, |v, _| {
        assert!(!v.pending());
        assert!(v.rates.is_some());
        assert_eq!(v.value("/devices/previewDevice"), "Headphones");
    });
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn unsaved_settings_draft_blocks_audio_start(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    assert!(model.read_with(cx, |m, cx| m.can_start(cx)));
    set(&view, window, "/voice/pitch", "3", cx);
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, cx| {
        m.has_unsaved_audio_draft(cx) && !m.can_start(cx)
    }));
    view.update(cx, |v, cx| v.save(CloseTarget::None, cx));
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, cx| {
        !m.has_unsaved_audio_draft(cx) && m.can_start(cx) && m.saved.pitch.semitones == 3.
    }));
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn asio_unsupported_sample_rate_rejects_options(cx: &mut TestAppContext) {
    use crate::backend::{DeviceChannels, DeviceSampleRates};

    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/devices/backend", "asio", cx);
    set(&view, window, "/devices/sampleRate", "44100", cx);
    view.update(cx, |v, _| {
        v.rates = Some(DeviceSampleRates {
            channels: vec![DeviceChannels {
                sample_rate: 48000,
                input: 2,
                output: 2,
            }],
            input: vec![48000],
            output: vec![48000],
            common: vec![48000],
        });
    });
    let error = view.read_with(cx, |v, _| v.options().unwrap_err());
    assert!(error.contains("/devices/sampleRate"));
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn opening_details_clears_save_failed_and_notice_when_switching_mode(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set(&view, window, "/inputEq/enabled", "true", cx);
    navigate(&model, window, "inputEq", cx);
    view.update(cx, |v, _| {
        v.save_failed = true;
        v.notice = "設定を保存しました。".into();
    });
    window
        .update(cx, |_, w, cx| w.click("details:inputEq", cx))
        .unwrap();
    cx.run_until_parked();
    view.read_with(cx, |v, _| {
        assert_eq!(v.value("/inputEq/mode"), "pro");
        assert!(!v.save_failed);
        assert!(v.notice.is_empty());
    });
    window
        .update(cx, |_, w, cx| w.click("close-details", cx))
        .unwrap();
    cx.run_until_parked();
    cleanup(model, window, cx);
}
