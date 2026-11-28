use super::{CloseTarget, Operation, SettingsView};
use crate::{
    backend::{BackendKind, DeviceChannels, DeviceSampleRates},
    cli::Options,
    gui::{
        draft::{
            DeviceSlot, EqBandField, EqKind, ErrorField, GateThresholdMode, NumberField, PitchUnit,
            ToggleField, UiMode,
        },
        layout,
        model::{AppModel, DeviceChoices, DeviceWarning},
        preferences::{Language, Preferences, Section},
    },
    pitch::PitchAlgorithm,
};
use gpui_kit::{
    test::TestWindowExt, AnyWindowHandle, AppContext, Entity, SharedString, TestAppContext,
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

pub(super) fn open(
    cx: &mut TestAppContext,
    directory: &std::path::Path,
) -> (Entity<AppModel>, AnyWindowHandle, Entity<SettingsView>) {
    open_with_options(cx, directory, Options::default())
}

fn open_with_options(
    cx: &mut TestAppContext,
    directory: &std::path::Path,
    options: Options,
) -> (Entity<AppModel>, AnyWindowHandle, Entity<SettingsView>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::gui::theme::init(cx);
    });
    let model = cx.update(|cx| {
        cx.new(|cx| {
            AppModel::new(
                options.clone(),
                options.clone(),
                directory.join("settings.ini"),
                Preferences {
                    language: Language::En,
                    section: Section::Voice,
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
                |w, cx| cx.new(|cx| SettingsView::new(model.clone(), options, w, cx)),
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

fn navigate(
    model: &Entity<AppModel>,
    window: AnyWindowHandle,
    section: Section,
    cx: &mut TestAppContext,
) {
    model.update(cx, |m, cx| {
        m.preferences.section = section;
        cx.notify();
    });
    cx.run_until_parked();
    window.update(cx, |_, w, cx| w.render_frame(cx)).unwrap();
}

pub(super) fn set_number(
    view: &Entity<SettingsView>,
    window: AnyWindowHandle,
    field: NumberField,
    text: &str,
    cx: &mut TestAppContext,
) {
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.draft.set_number_text(field, text.into());
                v.sync_number_widgets(field, w, cx);
                w.refresh();
                v.notify_host(cx);
                cx.notify();
            })
        })
        .unwrap();
    cx.run_until_parked();
}

fn set_toggle(
    view: &Entity<SettingsView>,
    window: AnyWindowHandle,
    field: ToggleField,
    value: bool,
    cx: &mut TestAppContext,
) {
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.draft.set_toggle(field, value);
                w.refresh();
                v.notify_host(cx);
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

fn drag_slider(
    window: AnyWindowHandle,
    field: NumberField,
    fraction: f32,
    cx: &mut TestAppContext,
) {
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            let bounds = w
                .find(SharedString::from(format!("slider:{}", field.id())))
                .bounds();
            let center = bounds.center();
            let end = gpui_kit::point(bounds.left() + bounds.size.width * fraction, center.y);
            w.drag(center, end, cx);
        })
        .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn failed_device_discovery_replaces_pending_status(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    model.update(cx, |m, cx| {
        m.preferences.section = Section::Devices;
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
    set_toggle(&view, window, ToggleField::NoiseGateEnabled, true, cx);
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.set_gate_threshold_mode(GateThresholdMode::Manual, w, cx);
            });
        })
        .unwrap();
    set_number(&view, window, NumberField::GateThresholdDb, "-35", cx);
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.set_gate_threshold_mode(GateThresholdMode::Auto, w, cx);
            });
        })
        .unwrap();
    set_number(&view, window, NumberField::Pitch, "7.50", cx);
    for kind in EqKind::ALL {
        for add in [true, false] {
            window
                .update(cx, |_, w, cx| {
                    view.update(cx, |v, cx| v.band_count(kind, add, w, cx));
                })
                .unwrap();
            cx.run_until_parked();
            assert_eq!(
                view.read_with(cx, |v, _| v
                    .draft
                    .number_text(NumberField::GateThresholdDb)
                    .to_string()),
                "-35"
            );
            assert_eq!(
                view.read_with(cx, |v, _| v
                    .draft
                    .number_text(NumberField::Pitch)
                    .to_string()),
                "7.50"
            );
        }
    }
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.set_gate_threshold_mode(GateThresholdMode::Manual, w, cx);
            });
        })
        .unwrap();
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
        (DeviceSlot::Input, ["", "Microphone A", "Microphone B"]),
        (DeviceSlot::Output, ["", "Output A", "Output B"]),
        (DeviceSlot::Preview, ["", "Output A", "Output B"]),
    ];
    view.update(cx, |v, _| {
        for (slot, items) in devices {
            v.selects.device_mut(slot).items = items.map(str::to_owned).into();
        }
    });
    for add in [true, false] {
        window
            .update(cx, |_, window, cx| {
                view.update(cx, |v, cx| {
                    v.band_count(EqKind::PostConversion, add, window, cx)
                });
            })
            .unwrap();
        view.read_with(cx, |v, _| {
            for (slot, items) in devices {
                assert_eq!(v.selects.device(slot).items.as_slice(), items);
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
    set_number(&view, window, NumberField::Pitch, "6.5", cx);
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
    set_number(&view, window, NumberField::Pitch, "6.5", cx);
    model.update(cx, |m, cx| {
        m.preferences.language = Language::Ja;
        m.preferences.section = Section::Devices;
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
    set_number(&view, window, NumberField::Pitch, "6.5", cx);
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
    assert!(view.read_with(cx, |v, _| v.error.is_some()
        && v.close == CloseTarget::None
        && !v.pending()));
    assert_eq!(
        view.read_with(cx, |v, _| v
            .draft
            .number_text(NumberField::Pitch)
            .to_string()),
        "6.5"
    );
    cleanup(model, window, cx);
    std::fs::remove_file(dir).unwrap();
}

#[gpui_kit::test]
fn overview_renders_active_stages_and_isolates_unfinished_fields(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    for field in [
        ToggleField::EqEnabled(EqKind::Input),
        ToggleField::EqEnabled(EqKind::PreConversion),
        ToggleField::EqEnabled(EqKind::PostConversion),
        ToggleField::LimiterEnabled,
        ToggleField::NoiseGateEnabled,
        ToggleField::DereverbEnabled,
        ToggleField::DeesserEnabled,
        ToggleField::CompressorEnabled,
        ToggleField::LevelEnabled,
        ToggleField::PostCompressorEnabled,
        ToggleField::ReverbEnabled,
    ] {
        set_toggle(&view, window, field, true, cx);
    }
    navigate(&model, window, Section::Overview, cx);
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            for stage in crate::audio::FILTER_STAGES {
                assert!(
                    w.try_find(SharedString::from(format!(
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
    set_number(&view, window, NumberField::CompressorRatio, "-", cx);
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("overview-plot:compressor:0").is_none());
            assert!(w.try_find("overview-plot:voice:0").is_some());
        })
        .unwrap();
    set_number(&view, window, NumberField::CompressorRatio, "3", cx);
    set_toggle(
        &view,
        window,
        ToggleField::EqEnabled(EqKind::Input),
        false,
        cx,
    );
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("overview-plot:inputEq:0").is_none());
        })
        .unwrap();
    set_toggle(&view, window, ToggleField::RumbleFilter, true, cx);
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
    let input = view.read_with(cx, |v, _| v.numbers[&NumberField::Pitch].input.clone());
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
    assert_eq!(
        view.read_with(cx, |v, _| v
            .draft
            .number_text(NumberField::Pitch)
            .to_string()),
        "7.5"
    );
    model.update(cx, |m, cx| {
        m.preferences.section = Section::Devices;
        cx.notify();
    });
    cx.run_until_parked();
    model.update(cx, |m, cx| {
        m.preferences.section = Section::Voice;
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
    set_number(&view, window, NumberField::Pitch, "8", cx);
    model.update(cx, |m, _| m.quit_requested = true);
    view.update(cx, |v, cx| {
        v.save(CloseTarget::Application, cx);
        v.save(CloseTarget::Application, cx);
    });
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.error.is_some() && !v.pending()));
    assert_eq!(
        view.read_with(cx, |v, _| v
            .draft
            .number_text(NumberField::Pitch)
            .to_string()),
        "8"
    );
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
    set_number(&view, settings, NumberField::Pitch, "", cx);
    main.update(cx, |_, window, _| window.activate_window())
        .unwrap();
    assert_eq!(cx.update(|cx| cx.active_window()), Some(main));

    model.update(cx, |m, cx| m.request_quit(cx));
    cx.run_until_parked();

    assert_eq!(cx.update(|cx| cx.active_window()), Some(settings));
    assert!(view.read_with(cx, |v, _| v.error.is_some()));
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
        m.open_settings(Some(Section::InputEq), cx);
    });
    assert_eq!(
        model.read_with(cx, |m, _| m.settings.as_ref().unwrap().0),
        first
    );
    assert_eq!(
        model.read_with(cx, |m, _| m.preferences.section),
        Section::InputEq
    );
    model.update(cx, |m, _| m.busy = false);
    view.update(cx, |v, cx| v.save(CloseTarget::Settings, cx));
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| m.settings.is_none()));
    model.update(cx, |m, cx| {
        m.busy = true;
        m.open_settings(Some(Section::Voice), cx);
    });
    cx.run_until_parked();
    let second = model.read_with(cx, |m, _| m.settings.as_ref().unwrap().0);
    assert_ne!(first, second);
    model.update(cx, |m, _| m.settings = None);
    second.update(cx, |_, w, _| w.remove_window()).unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}

#[gpui_kit::test]
fn eq_simple_slider_and_details_share_values_and_reset_preserves_shape(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    for kind in EqKind::ALL {
        let section = kind.section();
        set_toggle(&view, window, ToggleField::EqEnabled(kind), true, cx);
        navigate(&model, window, section, cx);
        let gain_field = NumberField::EqBand(kind, 0, EqBandField::GainDb);
        let freq_field = NumberField::EqBand(kind, 0, EqBandField::Frequency);
        let q_field = NumberField::EqBand(kind, 0, EqBandField::Q);
        let output_field = NumberField::EqOutputDb(kind);
        drag_slider(window, gain_field, 0.75, cx);
        let gain = view.read_with(cx, |v, _| {
            v.draft.number_text(gain_field).parse::<f64>().unwrap()
        });
        assert!(gain > 3. && gain <= 12., "{gain}");
        window
            .update(cx, |_, w, cx| {
                w.click(SharedString::from(format!("details:{}", section.id())), cx)
            })
            .unwrap();
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |v, _| v.draft.eq(kind).mode),
            UiMode::Pro
        );
        set_number(&view, window, freq_field, "420", cx);
        set_number(&view, window, q_field, "1.8", cx);
        set_number(&view, window, output_field, "-3.5", cx);
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
                w.click(SharedString::from(format!("reset:{}", section.id())), cx)
            })
            .unwrap();
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |v, _| v.draft.number_text(gain_field).to_string()),
            "0"
        );
        assert_eq!(
            view.read_with(cx, |v, _| v.draft.number_text(output_field).to_string()),
            "0"
        );
        assert_eq!(
            view.read_with(cx, |v, _| v.draft.number_text(freq_field).to_string()),
            "420"
        );
        assert_eq!(
            view.read_with(cx, |v, _| v.draft.number_text(q_field).to_string()),
            "1.8"
        );
        assert_eq!(
            view.read_with(cx, |v, _| v.draft.eq(kind).mode),
            UiMode::Pro
        );
        window
            .update(cx, |_, w, cx| w.click("close-details", cx))
            .unwrap();
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
    set_toggle(&view, window, ToggleField::DeesserEnabled, true, cx);
    navigate(&model, window, Section::Deesser, cx);
    window
        .update(cx, |_, w, cx| w.click("details:deesser", cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |v, _| v.draft.deesser_mode), UiMode::Pro);
    set_number(&view, window, NumberField::DeesserFrequencyHz, "7200", cx);
    window
        .update(cx, |_, w, cx| w.click("close-details", cx))
        .unwrap();
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.deesser_custom()));
    drag_slider(window, NumberField::DeesserRangeDb, 0.8, cx);
    assert_eq!(
        view.read_with(cx, |v, _| v
            .draft
            .number_text(NumberField::DeesserFrequencyHz)
            .to_string()),
        "7200"
    );
    assert!(
        view.read_with(cx, |v, _| v
            .draft
            .number_text(NumberField::DeesserRangeDb)
            .parse::<f64>()
            .unwrap())
            > 6.
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
    assert!(!view.read_with(cx, |v, _| v.deesser_custom()));
    assert_eq!(view.read_with(cx, |v, _| v.draft.deesser_mode), UiMode::Pro);
    window
        .update(cx, |_, w, cx| w.click("close-details", cx))
        .unwrap();
    cx.run_until_parked();
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn pitch_percent_does_not_change_formant_units_or_discard_an_unfinished_other_field(
    cx: &mut TestAppContext,
) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set_number(&view, window, NumberField::Pitch, "12", cx);
    set_number(&view, window, NumberField::FormantShift, "3", cx);
    set_number(&view, window, NumberField::CompressorRatio, "-", cx);
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| v.set_pitch_unit(PitchUnit::Percent, w, cx))
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |v, _| v
            .draft
            .number_text(NumberField::Pitch)
            .parse::<f64>()
            .unwrap()),
        200.
    );
    assert_eq!(
        view.read_with(cx, |v, _| v
            .draft
            .number_text(NumberField::FormantShift)
            .to_string()),
        "3"
    );
    assert_eq!(
        view.read_with(cx, |v, _| v
            .draft
            .number_text(NumberField::CompressorRatio)
            .to_string()),
        "-"
    );
    let range = view.read_with(cx, |v, cx| {
        let slider = v.numbers[&NumberField::FormantShift]
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
    assert_eq!(
        view.read_with(cx, |v, _| v
            .draft
            .number_text(NumberField::Pitch)
            .to_string()),
        "100"
    );
    assert_eq!(
        view.read_with(cx, |v, _| v
            .draft
            .number_text(NumberField::FormantShift)
            .to_string()),
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
                v.set_toggle(ToggleField::PreserveFormants, false, w, cx);
                v.set_pitch_algorithm(PitchAlgorithm::TdPsola, w, cx);
            })
        })
        .unwrap();
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.draft.preserve_formants));
    assert!(!view.read_with(cx, |v, cx| v
        .editable_toggle(ToggleField::PreserveFormants, cx)));
    set_toggle(&view, window, ToggleField::NoiseGateEnabled, true, cx);
    navigate(&model, window, Section::NoiseGate, cx);
    window
        .update(cx, |_, w, cx| w.click("details:noiseGate", cx))
        .unwrap();
    cx.run_until_parked();
    assert!(!view.read_with(cx, |v, cx| v
        .editable_number(NumberField::GateThresholdDb, cx)));
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.set_gate_threshold_mode(GateThresholdMode::Manual, w, cx)
            })
        })
        .unwrap();
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, cx| v
        .editable_number(NumberField::GateThresholdDb, cx)));
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn invalid_numeric_draft_disables_simple_slider_and_audio_locks_details(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set_toggle(&view, window, ToggleField::DeesserEnabled, true, cx);
    navigate(&model, window, Section::Deesser, cx);
    set_number(&view, window, NumberField::DeesserRangeDb, "-", cx);
    drag_slider(window, NumberField::DeesserRangeDb, 0.9, cx);
    assert_eq!(
        view.read_with(cx, |v, _| v
            .draft
            .number_text(NumberField::DeesserRangeDb)
            .to_string()),
        "-"
    );
    window
        .update(cx, |_, w, cx| w.click("details:deesser", cx))
        .unwrap();
    cx.run_until_parked();
    model.update(cx, |m, cx| {
        m.active = true;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(!view.read_with(cx, |v, cx| v
        .editable_number(NumberField::DeesserRangeDb, cx)));
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
    assert_eq!(
        view.read_with(cx, |v, _| v
            .draft
            .number_text(NumberField::DeesserRangeDb)
            .to_string()),
        "-"
    );
    model.update(cx, |m, _| m.active = false);
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn voice_sliders_and_numeric_arrows_stay_synchronized(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    drag_slider(window, NumberField::Pitch, 0.75, cx);
    let pitch = view.read_with(cx, |v, _| {
        v.draft
            .number_text(NumberField::Pitch)
            .parse::<f64>()
            .unwrap()
    });
    assert!((4.5..=7.5).contains(&pitch));
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| v.set_pitch_unit(PitchUnit::Percent, w, cx))
        })
        .unwrap();
    cx.run_until_parked();
    drag_slider(window, NumberField::FormantShift, 0.75, cx);
    let formant = view.read_with(cx, |v, _| {
        v.draft
            .number_text(NumberField::FormantShift)
            .parse::<f64>()
            .unwrap()
    });
    assert!((4.5..=7.5).contains(&formant));
    let input = view.read_with(cx, |v, _| {
        v.numbers[&NumberField::FormantShift].input.clone()
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
        v.draft
            .number_text(NumberField::FormantShift)
            .parse::<f64>()
            .unwrap()
    });
    assert!((number - formant - 0.01).abs() < 1e-8);
    let slider = view.read_with(cx, |v, cx| {
        v.numbers[&NumberField::FormantShift]
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
    set_toggle(&view, window, ToggleField::LevelEnabled, true, cx);
    set_toggle(&view, window, ToggleField::PostCompressorEnabled, true, cx);
    set_number(&view, window, NumberField::LevelTargetDb, "-16", cx);
    set_number(
        &view,
        window,
        NumberField::PostCompressorThresholdDb,
        "-22",
        cx,
    );
    navigate(&model, window, Section::AutoLevel, cx);
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
            .draft
            .number_text(NumberField::LevelTargetDb)
            .parse::<f64>()
            .unwrap()),
        -20.
    );
    assert_eq!(
        view.read_with(cx, |v, _| v
            .draft
            .number_text(NumberField::PostCompressorThresholdDb)
            .to_string()),
        "-22"
    );
    window
        .update(cx, |_, w, cx| w.click("close-details", cx))
        .unwrap();
    cx.run_until_parked();
    navigate(&model, window, Section::PostCompressor, cx);
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
            .draft
            .number_text(NumberField::PostCompressorThresholdDb)
            .parse::<f64>()
            .unwrap()),
        -12.
    );
    assert!(view.read_with(cx, |v, _| v.draft.level_enabled));
    assert!(view.read_with(cx, |v, _| v.draft.post_compressor_enabled));
    window
        .update(cx, |_, w, cx| w.click("close-details", cx))
        .unwrap();
    cx.run_until_parked();
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.set_synthesis_window(2048, w, cx);
                v.set_analysis_window(1024, w, cx);
            })
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |v, _| v.draft.synthesis_window), 1024);
    navigate(&model, window, Section::Voice, cx);
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
    set_toggle(
        &view,
        window,
        ToggleField::EqEnabled(EqKind::Input),
        true,
        cx,
    );
    set_number(
        &view,
        window,
        NumberField::EqBand(EqKind::Input, 0, EqBandField::Frequency),
        "380",
        cx,
    );
    set_number(
        &view,
        window,
        NumberField::EqBand(EqKind::Input, 0, EqBandField::GainDb),
        "-2.5",
        cx,
    );
    set_number(
        &view,
        window,
        NumberField::EqBand(EqKind::Input, 0, EqBandField::Q),
        "1.7",
        cx,
    );
    set_toggle(&view, window, ToggleField::DeesserEnabled, true, cx);
    set_number(&view, window, NumberField::DeesserFrequencyHz, "6500", cx);
    set_number(&view, window, NumberField::DeesserRangeDb, "9.5", cx);
    navigate(&model, window, Section::InputEq, cx);
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
        m.open_settings(Some(Section::InputEq), cx);
    });
    cx.run_until_parked();
    let (reopened, editor) = model.read_with(cx, |m, _| m.settings.as_ref().unwrap().clone());
    assert_eq!(
        editor.read_with(cx, |v, _| v
            .draft
            .number_text(NumberField::EqBand(EqKind::Input, 0, EqBandField::Q))
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
    set_toggle(&view, window, ToggleField::DeesserEnabled, true, cx);
    set_number(&view, window, NumberField::DeesserFrequencyHz, "6500", cx);
    navigate(&model, window, Section::Deesser, cx);
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
            view.read_with(cx, |v, _| v
                .draft
                .number_text(NumberField::DeesserFrequencyHz)
                .to_string()),
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
        set_number(&view, window, NumberField::Pitch, invalid, cx);
        window
            .update(cx, |_, w, cx| {
                view.update(cx, |v, cx| v.set_pitch_unit(PitchUnit::Percent, w, cx))
            })
            .unwrap();
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |v, _| v.draft.pitch_unit),
            PitchUnit::Semitones
        );
        assert_eq!(
            view.read_with(cx, |v, _| v
                .draft
                .number_text(NumberField::Pitch)
                .to_string()),
            invalid
        );
        assert!(view.read_with(cx, |v, _| v.error.is_some()));
    }
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn invalid_draft_can_be_discarded_on_quit(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set_number(&view, window, NumberField::Pitch, "-", cx);

    model.update(cx, |m, cx| m.request_quit(cx));
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.discard_target == CloseTarget::Application));
    assert!(model.read_with(cx, |m, _| !m.closing && !m.quit_requested));

    view.update(cx, |v, cx| v.discard_and_close(cx));
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| m.closing));
    assert!(!dir.exists());
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn close_retries_after_explicit_save_failure(cx: &mut TestAppContext) {
    let dir = directory();
    std::fs::write(&dir, "not a directory").unwrap();
    let (model, window, view) = open(cx, &dir);
    set_number(&view, window, NumberField::Pitch, "3", cx);

    view.update(cx, |v, cx| v.save(CloseTarget::None, cx));
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| {
        v.error.is_some() && v.discard_target == CloseTarget::None
    }));

    std::fs::remove_file(&dir).unwrap();
    view.update(cx, |v, cx| v.save(CloseTarget::Settings, cx));
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| m.settings.is_none()
        && m.saved.pitch.semitones == 3.));
    std::fs::remove_file(dir.join("settings.ini")).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

#[gpui_kit::test]
fn unmodified_settings_close_skips_disk_write_and_failed_close_allows_discard(
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
    set_number(&view, window, NumberField::Pitch, "6.5", cx);
    view.update(cx, |v, cx| v.save(CloseTarget::Settings, cx));
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| m.settings.is_some()));
    assert!(
        view.read_with(cx, |v, _| v.discard_target == CloseTarget::Settings
            && v.error.is_some())
    );

    view.update(cx, |v, cx| v.discard_and_close(cx));
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| m.settings.is_none()));
    assert!(window.update(cx, |_, _, _| ()).is_err());

    let (model, window, view) = open(cx, &dir);
    set_number(&view, window, NumberField::Pitch, "7", cx);
    view.update(cx, |v, cx| v.save(CloseTarget::Application, cx));
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| !m.closing));
    assert!(view.read_with(cx, |v, _| v.discard_target == CloseTarget::Application));

    view.update(cx, |v, cx| v.discard_and_close(cx));
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
            view.update(cx, |v, cx| v.set_pitch_unit(PitchUnit::Percent, w, cx))
        })
        .unwrap();
    assert!(view.read_with(cx, |v, _| v.notice.is_empty()));

    view.update(cx, |v, _| v.notice = "設定を保存しました。".into());
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.band_count(EqKind::PostConversion, true, w, cx)
            })
        })
        .unwrap();
    assert!(view.read_with(cx, |v, _| v.notice.is_empty()));
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn missing_device_and_unavailable_input_channel_are_labeled(cx: &mut TestAppContext) {
    use super::fields::device_choices;

    let en = Preferences {
        language: Language::En,
        section: Section::Devices,
    };
    let ja = Preferences {
        language: Language::Ja,
        section: Section::Devices,
    };
    let items = vec![String::new(), "OldMic".into()];
    let en_choices = device_choices(
        DeviceSlot::Input,
        &items,
        &en,
        BackendKind::Cpal,
        Some("OldMic"),
    );
    assert_eq!(
        en_choices[1].label,
        SharedString::from("OldMic (not detected)")
    );
    let ja_choices = device_choices(
        DeviceSlot::Input,
        &items,
        &ja,
        BackendKind::Cpal,
        Some("OldMic"),
    );
    assert_eq!(ja_choices[1].label, SharedString::from("OldMic（未検出）"));

    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| v.set_input_channel(4, w, cx));
        })
        .unwrap();
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
    navigate(&model, window, Section::Devices, cx);
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
        for slot in DeviceSlot::ALL {
            assert_eq!(v.selects.device(slot).items.as_slice(), &[String::new()]);
        }
    });
    view.update(cx, |v, _| {
        for (slot, items) in [
            (DeviceSlot::Input, vec![String::new(), "Old Mic".into()]),
            (
                DeviceSlot::Output,
                vec![String::new(), "Old Speaker".into()],
            ),
            (
                DeviceSlot::Preview,
                vec![String::new(), "Old Headphones".into()],
            ),
        ] {
            let select = v.selects.device_mut(slot);
            select.items = items;
            select.missing = Some("Old Missing".into());
        }
    });
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.draft.set_device(DeviceSlot::Input, "Old Mic".into());
                v.refresh_choices(w, cx);
            });
        })
        .unwrap();
    let current_backend = view.read_with(cx, |v, _| v.draft.backend);
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| v.set_backend(current_backend, w, cx));
        })
        .unwrap();
    view.read_with(cx, |v, _| {
        assert!(!v.pending());
        assert_eq!(v.draft.device(DeviceSlot::Input), "Old Mic");
        assert_eq!(
            v.selects.device(DeviceSlot::Input).items.as_slice(),
            &[String::new(), "Old Mic".into()]
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
        for slot in DeviceSlot::ALL {
            let select = v.selects.device(slot);
            assert_eq!(select.items.as_slice(), &[String::new()]);
            assert_eq!(select.missing, None);
            assert_eq!(v.draft.device(slot), "");
        }
    });
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn asio_discovery_ignores_unused_cpal_output_and_preview_devices(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    view.update(cx, |v, _| {
        v.draft.set_device(DeviceSlot::Input, "ASIO Driver".into());
        v.draft
            .set_device(DeviceSlot::Output, "Old CPAL Out".into());
        v.draft
            .set_device(DeviceSlot::Preview, "Old CPAL Preview".into());
    });
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
        for slot in DeviceSlot::ALL {
            assert_eq!(v.selects.device(slot).missing, None);
        }
    });
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn missing_device_message_is_not_overwritten_by_rate_lookup_failure(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    view.update(cx, |v, _| {
        v.draft.set_device(DeviceSlot::Input, "MissingMic".into());
    });
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
    navigate(&model, window, Section::Overview, cx);
    view.update(cx, |v, _| v.discard_target = CloseTarget::Settings);
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            w.click("stage-inputEq", cx);
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        model.read_with(cx, |m, _| m.preferences.section),
        Section::InputEq
    );
    assert!(view.read_with(cx, |v, _| v.discard_target == CloseTarget::None));

    view.update(cx, |v, _| v.discard_target = CloseTarget::Settings);
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            w.click("nav-overview", cx);
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        model.read_with(cx, |m, _| m.preferences.section),
        Section::Overview
    );
    assert!(view.read_with(cx, |v, _| v.discard_target == CloseTarget::None));

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
        model.read_with(cx, |m, _| m.preferences.section),
        Section::Overview
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
        model.read_with(cx, |m, _| m.preferences.section),
        Section::Overview
    );
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn out_of_range_input_and_output_channels_still_offer_valid_device_channels(
    cx: &mut TestAppContext,
) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.set_input_channel(4, w, cx);
                v.set_output_channels(vec![3, 4], w, cx);
            });
        })
        .unwrap();
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
    navigate(&model, window, Section::Devices, cx);
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
    assert_eq!(view.read_with(cx, |v, _| v.draft.input_channel), 1);
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn asio_unselected_shows_rate_hint_and_preview_device_change_skips_rediscovery(
    cx: &mut TestAppContext,
) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    view.update(cx, |v, _| {
        v.draft.backend = BackendKind::Asio;
        v.draft.device.clear();
    });
    navigate(&model, window, Section::Devices, cx);
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("device-select-hint").is_some());
        })
        .unwrap();

    view.update(cx, |v, cx| {
        v.draft.backend = BackendKind::Cpal;
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
                v.set_device(DeviceSlot::Preview, "Headphones".into(), w, cx);
            });
        })
        .unwrap();
    view.read_with(cx, |v, _| {
        assert!(!v.pending());
        assert!(v.rates.is_some());
        assert_eq!(v.draft.device(DeviceSlot::Preview), "Headphones");
    });
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn unsaved_settings_draft_blocks_audio_start(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open_with_options(
        cx,
        &dir,
        Options {
            backend: crate::backend::BackendKind::Cpal,
            pitch_unit: "percent".into(),
            pitch: crate::pitch::PitchSettings {
                semitones: 3.,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(model.read_with(cx, |m, cx| m.can_start(cx)));
    set_number(&view, window, NumberField::Pitch, "200", cx);
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, cx| {
        m.has_unsaved_audio_draft(cx) && !m.can_start(cx)
    }));
    view.update(cx, |v, cx| v.save(CloseTarget::None, cx));
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, cx| {
        !m.has_unsaved_audio_draft(cx) && m.can_start(cx) && m.saved.pitch.semitones == 12.
    }));
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn percent_pitch_unmodified_open_does_not_block_audio_start(cx: &mut TestAppContext) {
    let dir = directory();
    let options = Options {
        backend: BackendKind::Cpal,
        pitch_unit: "percent".into(),
        pitch: crate::pitch::PitchSettings {
            semitones: 7.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let (model, window, _view) = open_with_options(cx, &dir, options);
    assert!(model.read_with(cx, |m, cx| {
        !m.has_unsaved_audio_draft(cx) && m.can_start(cx)
    }));
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn asio_unsupported_sample_rate_rejects_options(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    view.update(cx, |v, _| {
        v.draft.backend = BackendKind::Asio;
        v.draft.sample_rate = 44100;
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
    assert_eq!(error.field, Some(ErrorField::SampleRate));
    assert_eq!(
        model.read_with(cx, |m, cx| m.start_blocked_message(cx)),
        Some("選択したサンプリングレートには対応していません。")
    );
    assert!(model.read_with(cx, |m, cx| {
        m.has_unsaved_audio_draft(cx) && !m.can_start(cx)
    }));

    view.update(cx, |v, cx| v.save(CloseTarget::Settings, cx));
    assert!(view.read_with(cx, |v, _| v.discard_target == CloseTarget::Settings));

    let saved = view.read_with(cx, |v, _| v.draft.to_options().unwrap());
    model.update(cx, |m, _| {
        m.saved = saved.clone();
        m.persisted_options = saved;
    });
    assert_eq!(
        model.read_with(cx, |m, cx| m.start_blocked_message(cx)),
        Some("選択したサンプリングレートには対応していません。")
    );
    view.update(cx, |v, cx| v.save(CloseTarget::Settings, cx));
    cx.run_until_parked();
    assert!(model.read_with(cx, |m, _| m.settings.is_none()));
    assert!(!dir.exists());
    assert!(window.update(cx, |_, _, _| ()).is_err());
}

#[gpui_kit::test]
fn opening_details_clears_discard_target_and_notice_when_switching_mode(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set_toggle(
        &view,
        window,
        ToggleField::EqEnabled(EqKind::Input),
        true,
        cx,
    );
    navigate(&model, window, Section::InputEq, cx);
    view.update(cx, |v, _| {
        v.discard_target = CloseTarget::Settings;
        v.notice = "設定を保存しました。".into();
    });
    window
        .update(cx, |_, w, cx| w.click("details:inputEq", cx))
        .unwrap();
    cx.run_until_parked();
    view.read_with(cx, |v, _| {
        assert_eq!(v.draft.eq(EqKind::Input).mode, UiMode::Pro);
        assert!(v.discard_target == CloseTarget::None);
        assert!(v.notice.is_empty());
    });
    window
        .update(cx, |_, w, cx| w.click("close-details", cx))
        .unwrap();
    cx.run_until_parked();
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn simple_slider_is_reachable_by_tab_and_arrows_change_value(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set_toggle(&view, window, ToggleField::DeesserEnabled, true, cx);
    navigate(&model, window, Section::Deesser, cx);
    let field = NumberField::DeesserRangeDb;
    let before = view.read_with(cx, |v, _| {
        v.draft.number_text(field).parse::<f64>().unwrap()
    });
    let focus = view.read_with(cx, |v, _| {
        v.numbers[&field]
            .slider_focus
            .clone()
            .expect("simple slider focus")
    });
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            w.focus_next(cx);
            w.render_frame(cx);
            let mut reached = focus.is_focused(w);
            for _ in 0..64 {
                if reached {
                    break;
                }
                w.press("tab", cx);
                w.render_frame(cx);
                reached = focus.is_focused(w);
            }
            assert!(reached, "SimpleSlider should be a Tab stop");
            w.press("right", cx);
        })
        .unwrap();
    cx.run_until_parked();
    let after = view.read_with(cx, |v, _| {
        v.draft.number_text(field).parse::<f64>().unwrap()
    });
    assert!(after > before, "before={before} after={after}");

    set_number(&view, window, field, "-", cx);
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            focus.focus(w, cx);
            w.render_frame(cx);
            assert!(focus.is_focused(w));
            w.press("right", cx);
            w.press("left", cx);
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |v, _| v.draft.number_text(field).to_string()),
        "-"
    );
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn simple_slider_thumb_click_focuses_and_arrows_change_value(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set_toggle(&view, window, ToggleField::DeesserEnabled, true, cx);
    navigate(&model, window, Section::Deesser, cx);
    let field = NumberField::DeesserRangeDb;
    let before = view.read_with(cx, |v, _| {
        v.draft.number_text(field).parse::<f64>().unwrap()
    });
    let focus = view.read_with(cx, |v, _| {
        v.numbers[&field]
            .slider_focus
            .clone()
            .expect("simple slider focus")
    });
    let slider_id = SharedString::from(format!("slider:{}", field.id()));
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            w.focus_next(cx);
            w.render_frame(cx);
            if focus.is_focused(w) {
                w.press("tab", cx);
                w.render_frame(cx);
            }
            assert!(!focus.is_focused(w), "focus must start off the slider");
            w.within(slider_id).click(("slider-thumb", 0u32), cx);
            w.render_frame(cx);
            assert!(
                focus.is_focused(w),
                "clicking SliderThumb should focus SimpleSlider"
            );
            w.press("right", cx);
        })
        .unwrap();
    cx.run_until_parked();
    let after = view.read_with(cx, |v, _| {
        v.draft.number_text(field).parse::<f64>().unwrap()
    });
    assert!(after > before, "before={before} after={after}");
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn blocked_simple_slider_skips_tab_allows_leave_and_ignores_click(cx: &mut TestAppContext) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    set_toggle(&view, window, ToggleField::DeesserEnabled, true, cx);
    navigate(&model, window, Section::Deesser, cx);
    let field = NumberField::DeesserRangeDb;
    set_number(&view, window, field, "-", cx);
    let focus = view.read_with(cx, |v, _| {
        v.numbers[&field]
            .slider_focus
            .clone()
            .expect("simple slider focus")
    });
    let slider_id = SharedString::from(format!("slider:{}", field.id()));
    window
        .update(cx, |_, w, cx| {
            w.render_frame(cx);
            w.focus_next(cx);
            w.render_frame(cx);
            for _ in 0..64 {
                assert!(
                    !focus.is_focused(w),
                    "blocked SimpleSlider must not be a Tab stop"
                );
                w.press("tab", cx);
                w.render_frame(cx);
            }
            focus.focus(w, cx);
            w.render_frame(cx);
            assert!(focus.is_focused(w));
            w.press("tab", cx);
            w.render_frame(cx);
            assert!(
                !focus.is_focused(w),
                "blocked && focused SimpleSlider must leave on Tab"
            );
            // Disabled SliderThumb is not hittable; click the tracked wrapper instead.
            w.click(slider_id, cx);
            w.render_frame(cx);
            assert!(
                !focus.is_focused(w),
                "click must not refocus a blocked SimpleSlider"
            );
            w.press("right", cx);
            w.press("left", cx);
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |v, _| v.draft.number_text(field).to_string()),
        "-"
    );
    drag_slider(window, field, 0.9, cx);
    assert_eq!(
        view.read_with(cx, |v, _| v.draft.number_text(field).to_string()),
        "-"
    );
    cleanup(model, window, cx);
}

#[gpui_kit::test]
fn set_device_missing_detection_uses_discovered_across_alias_and_missing_round_trips(
    cx: &mut TestAppContext,
) {
    let dir = directory();
    let (model, window, view) = open(cx, &dir);
    let labeled = "Headphones [out-2]";
    let missing_msg = DeviceWarning::Missing.message();
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.draft.backend = BackendKind::Cpal;
                v.apply_discovery(
                    Ok((
                        DeviceChoices {
                            inputs: vec!["Mic [in-1]".into()],
                            outputs: vec![labeled.into()],
                            resolved: BackendKind::Cpal,
                        },
                        Ok(None),
                    )),
                    w,
                    cx,
                );
            });
        })
        .unwrap();
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.set_device(DeviceSlot::Preview, "Headphones".into(), w, cx);
            });
        })
        .unwrap();
    view.read_with(cx, |v, _| {
        let select = v.selects.device(DeviceSlot::Preview);
        assert_eq!(select.discovered.as_slice(), &[labeled.to_string()]);
        assert_eq!(select.missing, None);
        assert!(v.device_error.is_empty());
        assert_eq!(
            select.items.as_slice(),
            &[String::new(), labeled.into(), "Headphones".into()]
        );
    });
    // Reselecting the CPAL alias must not flip the device to missing.
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.set_device(DeviceSlot::Preview, "Headphones".into(), w, cx);
            });
        })
        .unwrap();
    view.read_with(cx, |v, _| {
        assert_eq!(v.selects.device(DeviceSlot::Preview).missing, None);
        assert!(v.device_error.is_empty());
    });
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.set_device(DeviceSlot::Preview, "Speaker [out-1]".into(), w, cx);
                v.set_device(DeviceSlot::Preview, "Headphones".into(), w, cx);
            });
        })
        .unwrap();
    view.read_with(cx, |v, _| {
        assert_eq!(v.selects.device(DeviceSlot::Preview).missing, None);
        assert!(v.device_error.is_empty());
    });

    // Missing preview must leave items after switching away, and stay missing if reselected.
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.set_device(DeviceSlot::Preview, "OldHeadphones".into(), w, cx);
            });
        })
        .unwrap();
    view.read_with(cx, |v, _| {
        let select = v.selects.device(DeviceSlot::Preview);
        assert_eq!(select.missing.as_deref(), Some("OldHeadphones"));
        assert_eq!(v.device_error, missing_msg);
        assert_eq!(
            select.items.as_slice(),
            &[String::new(), labeled.into(), "OldHeadphones".into()]
        );
    });
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.set_device(DeviceSlot::Preview, String::new(), w, cx);
            });
        })
        .unwrap();
    view.read_with(cx, |v, _| {
        let select = v.selects.device(DeviceSlot::Preview);
        assert_eq!(select.missing, None);
        assert!(v.device_error.is_empty());
        assert_eq!(select.items.as_slice(), &[String::new(), labeled.into()]);
        assert!(!select.items.iter().any(|item| item == "OldHeadphones"));
    });
    window
        .update(cx, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.set_device(DeviceSlot::Preview, "OldHeadphones".into(), w, cx);
            });
        })
        .unwrap();
    view.read_with(cx, |v, _| {
        assert_eq!(
            v.selects.device(DeviceSlot::Preview).missing.as_deref(),
            Some("OldHeadphones")
        );
        assert_eq!(v.device_error, missing_msg);
    });
    cleanup(model, window, cx);
}
