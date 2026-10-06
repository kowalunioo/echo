//! `tray.md` acceptance tests 1–6, 9 and 10 against a fake tray, the fake Engine, the WAV Audio
//! Source and the fake Inserter.

use std::cell::RefCell;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::actions::{ActionTarget, perform};
use super::theme::{ThemeSource, ThemeWatcher};
use super::*;
use crate::audio::{AudioSource, Pace, WavAudioSource, WavOptions};
use crate::dictation::vad::tests::EnergyDetector;
use crate::dictation::{
    Dictation, DictationContext, DictationDeps, DictationModel, DictationModels, EngineModel,
};
use crate::engine::{EngineError, FakeEngine};
use crate::history::{EntryLanguage, History, HistoryStore, NewEntry};
use crate::insertion::{FakeInserter, SharedInserter};
use crate::models::NoActiveModel;
use crate::overlay::app::MainPage;
use crate::shortcut::modes::RecordIntent::{Start, Stop};

const VERSION: &str = "0.1.0";

/// What the fake tray was told, in order.
#[derive(Default)]
struct Shown {
    icons: Vec<(TrayIconState, TaskbarTheme)>,
    tooltips: Vec<String>,
    menus: Vec<Vec<MenuEntry>>,
}

#[derive(Clone, Default)]
struct FakeTray(Arc<Mutex<Shown>>);

impl FakeTray {
    fn icon(&self) -> TrayIconState {
        self.0.lock().unwrap().icons.last().unwrap().0
    }
    fn icon_states(&self) -> Vec<TrayIconState> {
        let mut states: Vec<TrayIconState> = Vec::new();
        for (state, _) in &self.0.lock().unwrap().icons {
            if states.last() != Some(state) {
                states.push(*state);
            }
        }
        states
    }
    fn theme(&self) -> TaskbarTheme {
        self.0.lock().unwrap().icons.last().unwrap().1
    }
    fn tooltip(&self) -> String {
        self.0.lock().unwrap().tooltips.last().unwrap().clone()
    }
    fn menu(&self) -> Vec<MenuEntry> {
        self.0.lock().unwrap().menus.last().unwrap().clone()
    }
}

impl TrayView for FakeTray {
    fn show_icon(&mut self, state: TrayIconState, theme: TaskbarTheme) {
        self.0.lock().unwrap().icons.push((state, theme));
    }
    fn show_tooltip(&mut self, text: &str) {
        self.0.lock().unwrap().tooltips.push(text.to_owned());
    }
    fn show_menu(&mut self, entries: &[MenuEntry]) {
        self.0.lock().unwrap().menus.push(entries.to_vec());
    }
}

fn inputs() -> TrayInputs {
    TrayInputs {
        status: DictationStatus::default(),
        theme: TaskbarTheme::Dark,
        language: UiLanguage::En,
        models: MenuModels {
            downloaded: vec![
                (
                    ModelId::WhisperLargeV3Turbo,
                    "Whisper large-v3-turbo".into(),
                ),
                (ModelId::WhisperSmall, "Whisper small".into()),
            ],
            active: Some(ModelId::WhisperLargeV3Turbo),
            busy: false,
        },
        history_empty: false,
        updates_enabled: true,
    }
}

fn status(state: DictationState, error: Option<ProblemKind>) -> DictationStatus {
    DictationStatus {
        state,
        error: error.map(|kind| DictationProblem {
            id: 1,
            kind,
            detail: String::new(),
        }),
        ..DictationStatus::default()
    }
}

fn labels(entries: &[MenuEntry]) -> Vec<String> {
    entries
        .iter()
        .map(|entry| match entry {
            MenuEntry::Info(text) => text.clone(),
            MenuEntry::Separator => "---".into(),
            MenuEntry::Item { label, .. } | MenuEntry::Models { label, .. } => label.clone(),
        })
        .collect()
}

fn item(entries: &[MenuEntry], wanted: TrayAction) -> Option<(String, bool)> {
    entries.iter().find_map(|entry| match entry {
        MenuEntry::Item {
            action,
            label,
            enabled,
        } if *action == wanted => Some((label.clone(), *enabled)),
        _ => None,
    })
}

fn models_entry(entries: &[MenuEntry]) -> (String, bool, Vec<ModelChoice>) {
    entries
        .iter()
        .find_map(|entry| match entry {
            MenuEntry::Models {
                label,
                enabled,
                choices,
            } => Some((label.clone(), *enabled, choices.clone())),
            _ => None,
        })
        .unwrap()
}

// ---- The pipeline against fakes, feeding a controller with a fake tray. ----

fn wav(speech_ms: u32, silence_ms: u32) -> Vec<u8> {
    let rate = 48_000;
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut bytes = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut bytes, spec).unwrap();
    let speech = (rate / 1000 * speech_ms) as usize;
    for i in 0..speech + (rate / 1000 * silence_ms) as usize {
        let s = if i < speech {
            ((i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 16_000.0) as i16
        } else {
            0
        };
        writer.write_sample(s).unwrap();
        writer.write_sample(s).unwrap();
    }
    writer.finalize().unwrap();
    bytes.into_inner()
}

/// A whole Dictation in one go: the source ends after 1.2 s of tone.
fn finite_source() -> WavAudioSource {
    WavAudioSource::from_reader(
        Cursor::new(wav(1200, 200)),
        WavOptions {
            pace: Pace::AsFastAsPossible,
            stop_at_end: true,
        },
    )
    .unwrap()
}

/// A Recording that lasts until it is stopped or cancelled.
fn endless_source() -> WavAudioSource {
    WavAudioSource::from_reader(
        Cursor::new(wav(1200, 0)),
        WavOptions {
            pace: Pace::RealTime,
            stop_at_end: false,
        },
    )
    .unwrap()
}

struct Models(Arc<EngineModel<FakeEngine>>);

impl DictationModels for Models {
    fn begin(&self) -> Result<Arc<dyn DictationModel>, NoActiveModel> {
        Ok(Arc::clone(&self.0) as Arc<dyn DictationModel>)
    }
}

struct Rig {
    dictation: Dictation,
    tray: FakeTray,
    inserter: FakeInserter,
    stored: Arc<Mutex<Vec<String>>>,
}

fn rig(engine: FakeEngine, source: impl AudioSource + 'static) -> Rig {
    let tray = FakeTray::default();
    let mut idle = inputs();
    idle.status = DictationStatus::default();
    let controller = Arc::new(Mutex::new(TrayController::new(tray.clone(), VERSION, idle)));
    let inserter = FakeInserter::new();
    let shared = SharedInserter::new();
    shared.set(inserter.clone());
    let stored = Arc::new(Mutex::new(Vec::new()));
    let history = Arc::clone(&stored);
    let dictation = Dictation::spawn(DictationDeps {
        models: Box::new(Models(Arc::new(EngineModel::new("Fake", engine)))),
        source: Box::new(source),
        detector: Box::new(|| Box::new(EnergyDetector)),
        context: Box::new(DictationContext::default),
        history: Box::new(move |entry: NewEntry| {
            history.lock().unwrap().push(entry.text);
            Ok(())
        }),
        inserter: shared,
        shortcut_reset: Box::new(|| {}),
        publish: Box::new(move |status| {
            controller
                .lock()
                .unwrap()
                .apply(TrayUpdate::Status(status.clone()));
        }),
        max_recording: Duration::from_secs(600),
    });
    Rig {
        dictation,
        tray,
        inserter,
        stored,
    }
}

impl Rig {
    fn wait(&self, what: &str, check: impl Fn(&DictationStatus) -> bool) {
        let until = Instant::now() + Duration::from_secs(5);
        while !check(&self.dictation.status()) {
            assert!(Instant::now() < until, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// One complete Dictation; returns once it is Idle again.
    fn dictate(&self) {
        let shown = || self.tray.0.lock().unwrap().icons.len();
        let before = shown();
        self.dictation.intent(Start);
        // The fast source can finish before a status poll sees it, so follow the tray instead.
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let ended = shown() > before
                && matches!(self.tray.icon(), TrayIconState::Idle | TrayIconState::Error)
                && self.dictation.status().state == DictationState::Idle;
            if ended {
                return;
            }
            assert!(
                Instant::now() < until,
                "timed out waiting for the Dictation"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

#[test]
fn acceptance_1_the_icon_follows_the_dictation_state() {
    let rig = rig(
        FakeEngine::returning("Ala ma kota.").with_delay(Duration::from_millis(100)),
        finite_source(),
    );

    rig.dictate();

    assert_eq!(rig.inserter.inserted(), ["Ala ma kota."]);
    assert_eq!(
        rig.tray.icon_states(),
        [
            TrayIconState::Idle,
            TrayIconState::Recording,
            TrayIconState::Transcribing,
            TrayIconState::Idle
        ]
    );
}

#[test]
fn inserting_shows_transcribing_and_dictation_icons_win_over_an_error() {
    use DictationState::*;
    assert_eq!(icon_state(&status(Idle, None)), TrayIconState::Idle);
    assert_eq!(
        icon_state(&status(Recording, None)),
        TrayIconState::Recording
    );
    assert_eq!(
        icon_state(&status(Transcribing, None)),
        TrayIconState::Transcribing
    );
    assert_eq!(
        icon_state(&status(Inserting, None)),
        TrayIconState::Transcribing
    );
    let failed = Some(ProblemKind::TranscriptionFailed);
    assert_eq!(icon_state(&status(Idle, failed)), TrayIconState::Error);
    assert_eq!(
        icon_state(&status(Recording, failed)),
        TrayIconState::Recording
    );
    assert_eq!(
        icon_state(&status(Transcribing, failed)),
        TrayIconState::Transcribing
    );
    assert_eq!(icon_state(&status(Inserting, failed)), TrayIconState::Error);
}

/// A taskbar theme the test switches.
#[derive(Clone)]
struct FakeTheme(Arc<Mutex<TaskbarTheme>>);

impl ThemeSource for FakeTheme {
    fn taskbar_theme(&self) -> TaskbarTheme {
        *self.0.lock().unwrap()
    }
}

#[test]
fn acceptance_2_the_icon_variant_follows_the_taskbar_theme() {
    let theme = FakeTheme(Arc::new(Mutex::new(TaskbarTheme::Light)));
    let mut watcher = ThemeWatcher::new(theme.clone());
    let tray = FakeTray::default();
    let mut start = inputs();
    start.theme = watcher.current();
    let mut controller = TrayController::new(tray.clone(), VERSION, start);
    assert_eq!(tray.theme(), TaskbarTheme::Light);
    assert_eq!(watcher.poll(), None);

    *theme.0.lock().unwrap() = TaskbarTheme::Dark;
    let changed = watcher.poll().expect("a theme change");
    controller.apply(TrayUpdate::Theme(changed));
    assert_eq!(tray.theme(), TaskbarTheme::Dark);
    assert_eq!(watcher.poll(), None);

    *theme.0.lock().unwrap() = TaskbarTheme::Light;
    controller.apply(TrayUpdate::Theme(watcher.poll().unwrap()));
    assert_eq!(tray.theme(), TaskbarTheme::Light);
}

#[test]
fn a_light_taskbar_gets_the_dark_glyph_and_every_glyph_has_its_size() {
    for state in [
        TrayIconState::Idle,
        TrayIconState::Recording,
        TrayIconState::Transcribing,
        TrayIconState::Error,
    ] {
        for theme in [TaskbarTheme::Light, TaskbarTheme::Dark] {
            for size in icons::SIZES {
                let image = tauri::image::Image::from_bytes(icons::png(state, theme, size))
                    .expect("a valid PNG");
                assert_eq!((image.width(), image.height()), (size, size));
            }
        }
        let on_light = icons::png(state, TaskbarTheme::Light, 16);
        let on_dark = icons::png(state, TaskbarTheme::Dark, 16);
        assert_ne!(on_light, on_dark);
    }
    // Opaque pixels of the idle glyph on a light taskbar are dark, and light on a dark one.
    let brightness = |theme| {
        let image =
            tauri::image::Image::from_bytes(icons::png(TrayIconState::Idle, theme, 32)).unwrap();
        let rgba = image.rgba();
        let opaque: Vec<_> = rgba.chunks(4).filter(|p| p[3] == 255).collect();
        opaque.iter().map(|p| u32::from(p[0])).sum::<u32>() / opaque.len() as u32
    };
    assert!(brightness(TaskbarTheme::Light) < 64);
    assert!(brightness(TaskbarTheme::Dark) > 192);
}

#[test]
fn the_glyph_size_matches_the_display_scale_without_scaling_up() {
    assert_eq!(icons::size_for_scale(1.0), 16);
    assert_eq!(icons::size_for_scale(1.25), 20);
    assert_eq!(icons::size_for_scale(1.5), 24);
    assert_eq!(icons::size_for_scale(1.75), 32);
    assert_eq!(icons::size_for_scale(2.0), 32);
    assert_eq!(icons::size_for_scale(3.0), 32);
}

#[test]
fn acceptance_2a_an_engine_failure_turns_the_icon_red_until_the_window_is_seen() {
    let rig = rig(
        FakeEngine::failing(EngineError::Transcription("GPU lost".into())),
        finite_source(),
    );

    rig.dictate();

    assert_eq!(rig.tray.icon(), TrayIconState::Error);
    assert_eq!(rig.tray.tooltip(), "Echo 0.1.0 — Transcription failed");
    rig.dictation.window_seen();
    rig.wait("the error to clear", |s| s.error.is_none());
    assert_eq!(rig.tray.icon(), TrayIconState::Idle);
    assert_eq!(rig.tray.tooltip(), "Echo 0.1.0");
}

#[test]
fn acceptance_2a_a_later_successful_dictation_clears_the_error_but_a_cancelled_one_does_not() {
    let engine = FakeEngine::failing(EngineError::Transcription("x".into()));
    let rig = rig(engine.clone(), endless_source());
    let dictate = || {
        rig.dictation.intent(Start);
        rig.wait("Recording", |s| {
            s.state == DictationState::Recording && s.listening
        });
        std::thread::sleep(Duration::from_millis(1300));
        rig.dictation.intent(Stop);
        rig.wait("Idle", |s| s.state == DictationState::Idle);
    };
    dictate();
    assert_eq!(rig.tray.icon(), TrayIconState::Error);

    // A cancelled Dictation shows the recording icon, then error again.
    rig.dictation.intent(Start);
    rig.wait("Recording", |s| s.state == DictationState::Recording);
    assert_eq!(rig.tray.icon(), TrayIconState::Recording);
    rig.dictation.cancel();
    rig.wait("Idle", |s| s.state == DictationState::Idle);
    assert_eq!(rig.tray.icon(), TrayIconState::Error);

    engine.set_outcome(Ok("Dobrze.".into()));
    dictate();
    assert_eq!(rig.inserter.inserted(), ["Dobrze."]);
    assert_eq!(rig.tray.icon(), TrayIconState::Idle);
}

#[test]
fn acceptance_2b_a_failed_model_download_turns_the_icon_red_naming_the_model() {
    let rig = rig(FakeEngine::returning("x"), finite_source());

    rig.dictation.report_problem(
        ProblemKind::ModelDownloadFailed,
        "Whisper small: the connection was reset",
    );
    rig.wait("the error", |s| s.error.is_some());

    assert_eq!(rig.tray.icon(), TrayIconState::Error);
    assert_eq!(
        rig.tray.tooltip(),
        "Echo 0.1.0 — Model download failed (Whisper small)"
    );
}

#[test]
fn the_tooltip_names_the_state_and_follows_the_ui_language() {
    use DictationState::*;
    let en = UiLanguage::En;
    assert_eq!(tooltip(en, VERSION, &status(Idle, None)), "Echo 0.1.0");
    assert_eq!(
        tooltip(en, VERSION, &status(Recording, None)),
        "Echo 0.1.0 — recording"
    );
    assert_eq!(
        tooltip(en, VERSION, &status(Inserting, None)),
        "Echo 0.1.0 — transcribing"
    );
    assert_eq!(
        tooltip(
            UiLanguage::Pl,
            VERSION,
            &status(Idle, Some(ProblemKind::NoModel))
        ),
        "Echo 0.1.0 — Brak Modelu"
    );
}

#[test]
fn acceptance_3_while_busy_the_menu_offers_cancel_and_locks_the_model() {
    let mut busy = inputs();
    busy.status = status(DictationState::Recording, None);
    let entries = menu(VERSION, &busy);
    assert_eq!(
        labels(&entries),
        [
            "Echo 0.1.0",
            "---",
            "Cancel",
            "---",
            "Copy last Transcript",
            "---",
            "Whisper large-v3-turbo",
            "---",
            "Settings…",
            "Check for updates…",
            "---",
            "Quit Echo"
        ]
    );
    assert!(!models_entry(&entries).1);

    let idle = menu(VERSION, &inputs());
    assert_eq!(item(&idle, TrayAction::Cancel), None);
    assert_eq!(
        labels(&idle),
        [
            "Echo 0.1.0",
            "---",
            "Copy last Transcript",
            "---",
            "Whisper large-v3-turbo",
            "---",
            "Settings…",
            "Check for updates…",
            "---",
            "Quit Echo"
        ]
    );
    let (_, enabled, choices) = models_entry(&idle);
    assert!(enabled);
    assert_eq!(
        choices
            .iter()
            .map(|c| (c.model, c.active))
            .collect::<Vec<_>>(),
        [
            (ModelId::WhisperLargeV3Turbo, true),
            (ModelId::WhisperSmall, false)
        ]
    );
    assert!(matches!(&idle[0], MenuEntry::Info(_)));
}

#[test]
fn the_model_submenu_is_disabled_without_downloaded_models_or_while_the_manager_is_busy() {
    let mut none = inputs();
    none.models = MenuModels::default();
    let (label, enabled, choices) = models_entry(&menu(VERSION, &none));
    assert_eq!((label.as_str(), enabled), ("No Model downloaded", false));
    assert!(choices.is_empty());

    let mut busy = inputs();
    busy.models.busy = true;
    assert!(!models_entry(&menu(VERSION, &busy)).1);

    let mut inactive = inputs();
    inactive.models.active = None;
    assert_eq!(models_entry(&menu(VERSION, &inactive)).0, "Model");
}

#[test]
fn menu_item_ids_round_trip() {
    let mut actions = vec![
        TrayAction::Cancel,
        TrayAction::CopyLastTranscript,
        TrayAction::Settings,
        TrayAction::CheckForUpdates,
        TrayAction::Quit,
    ];
    actions.extend(ModelId::ALL.map(TrayAction::ActivateModel));
    for action in actions {
        assert_eq!(TrayAction::from_id(&action.id()), Some(action));
    }
    assert_eq!(TrayAction::from_id("info"), None);
    assert_eq!(TrayAction::from_id("model-9"), None);
}

#[test]
fn acceptance_10_menu_labels_follow_the_ui_language() {
    let tray = FakeTray::default();
    let mut controller = TrayController::new(tray.clone(), VERSION, inputs());

    controller.apply(TrayUpdate::Language(UiLanguage::Pl));

    let entries = tray.menu();
    assert_eq!(item(&entries, TrayAction::Quit).unwrap().0, "Zakończ Echo");
    assert_eq!(
        item(&entries, TrayAction::Settings).unwrap().0,
        "Ustawienia…"
    );
    assert_eq!(
        item(&entries, TrayAction::CopyLastTranscript).unwrap().0,
        "Kopiuj ostatnią transkrypcję"
    );
}

#[test]
fn the_controller_only_touches_what_changed() {
    let tray = FakeTray::default();
    let mut controller = TrayController::new(tray.clone(), VERSION, inputs());
    let counts = |tray: &FakeTray| {
        let shown = tray.0.lock().unwrap();
        (shown.icons.len(), shown.tooltips.len(), shown.menus.len())
    };
    assert_eq!(counts(&tray), (1, 1, 1));

    controller.apply(TrayUpdate::Status(DictationStatus {
        listening: true,
        ..status(DictationState::Recording, None)
    }));
    assert_eq!(counts(&tray), (2, 2, 2));
    controller.apply(TrayUpdate::Status(status(DictationState::Recording, None)));
    assert_eq!(counts(&tray), (2, 2, 2));
    controller.apply(TrayUpdate::Theme(TaskbarTheme::Light));
    assert_eq!(counts(&tray), (3, 2, 2));
    controller.apply(TrayUpdate::Refresh);
    assert_eq!(counts(&tray), (4, 2, 3));
}

/// The app side of the menu, with the real pipeline and History.
struct Target<'a> {
    dictation: &'a Dictation,
    history: &'a History,
    clipboard: RefCell<Vec<String>>,
    activated: RefCell<Vec<ModelId>>,
    shown: RefCell<Vec<MainPage>>,
    update_checks: RefCell<usize>,
    exited: RefCell<bool>,
}

impl<'a> Target<'a> {
    fn new(dictation: &'a Dictation, history: &'a History) -> Self {
        Self {
            dictation,
            history,
            clipboard: RefCell::default(),
            activated: RefCell::default(),
            shown: RefCell::default(),
            update_checks: RefCell::default(),
            exited: RefCell::default(),
        }
    }
}

impl ActionTarget for Target<'_> {
    fn cancel_dictation(&self) {
        self.dictation.cancel();
    }
    fn latest_transcript(&self) -> Option<String> {
        self.history.latest().unwrap().map(|entry| entry.text)
    }
    fn copy_to_clipboard(&self, text: &str) -> Result<(), String> {
        self.clipboard.borrow_mut().push(text.to_owned());
        Ok(())
    }
    fn activate_model(&self, model: ModelId) {
        self.activated.borrow_mut().push(model);
    }
    fn open_main_page(&self, page: MainPage) {
        self.shown.borrow_mut().push(page);
    }
    fn check_for_updates(&self) {
        *self.update_checks.borrow_mut() += 1;
    }
    fn exit(&self) {
        *self.exited.borrow_mut() = true;
    }
}

fn history(texts: &[&str]) -> History {
    let history = History::new(HistoryStore::open_in_memory().unwrap(), 5).unwrap();
    for text in texts {
        history
            .add(NewEntry {
                text: (*text).to_owned(),
                language: EntryLanguage::Specific { code: "pl".into() },
                model: "Fake".into(),
            })
            .unwrap();
    }
    history
}

#[test]
fn acceptance_4_cancel_during_recording_is_a_cancellation() {
    let rig = rig(FakeEngine::returning("nie"), endless_source());
    let history = history(&[]);
    let target = Target::new(&rig.dictation, &history);
    rig.dictation.intent(Start);
    rig.wait("Recording", |s| {
        s.state == DictationState::Recording && s.listening
    });

    perform(TrayAction::Cancel, &target);

    rig.wait("Idle", |s| s.state == DictationState::Idle);
    std::thread::sleep(Duration::from_millis(200));
    assert!(rig.inserter.inserted().is_empty());
    assert!(rig.stored.lock().unwrap().is_empty());
    assert_eq!(rig.tray.icon(), TrayIconState::Idle);
    assert!(!*target.exited.borrow());
}

#[test]
fn acceptance_5_copy_last_transcript_copies_the_newest_entry() {
    let rig = rig(FakeEngine::returning("x"), finite_source());
    let filled = history(&["older", "newest"]);
    let target = Target::new(&rig.dictation, &filled);

    perform(TrayAction::CopyLastTranscript, &target);

    assert_eq!(*target.clipboard.borrow(), ["newest"]);

    let empty = history(&[]);
    let target = Target::new(&rig.dictation, &empty);
    perform(TrayAction::CopyLastTranscript, &target);
    assert!(target.clipboard.borrow().is_empty());
    let mut no_history = inputs();
    no_history.history_empty = true;
    assert_eq!(
        item(&menu(VERSION, &no_history), TrayAction::CopyLastTranscript),
        Some(("Copy last Transcript".into(), false))
    );
}

#[test]
fn acceptance_6_choosing_another_model_activates_it_and_settings_opens_the_app_page() {
    let rig = rig(FakeEngine::returning("x"), finite_source());
    let history = history(&[]);
    let target = Target::new(&rig.dictation, &history);

    perform(TrayAction::ActivateModel(ModelId::WhisperSmall), &target);
    perform(TrayAction::Settings, &target);

    assert_eq!(*target.activated.borrow(), [ModelId::WhisperSmall]);
    assert_eq!(*target.shown.borrow(), [MainPage::App]);
}

#[test]
fn acceptance_9_quit_during_recording_inserts_and_stores_nothing() {
    let rig = rig(FakeEngine::returning("nie"), endless_source());
    let history = history(&[]);
    let target = Target::new(&rig.dictation, &history);
    rig.dictation.intent(Start);
    rig.wait("Recording", |s| {
        s.state == DictationState::Recording && s.listening
    });

    perform(TrayAction::Quit, &target);

    assert!(*target.exited.borrow());
    rig.wait("Idle", |s| s.state == DictationState::Idle);
    std::thread::sleep(Duration::from_millis(200));
    assert!(rig.inserter.inserted().is_empty());
    assert!(rig.stored.lock().unwrap().is_empty());
}

// Closing the main window never exits Echo, even though the hidden Overlay window would let the
// app outlive it; only "Quit Echo" exits (rules 12-13).
#[test]
fn closing_the_main_window_shows_the_hint_once_then_hides() {
    assert_eq!(close_outcome(false), CloseOutcome::ShowHint);
    assert_eq!(close_outcome(true), CloseOutcome::Hide);
}

// updater.md "UI" and rule 11.
#[test]
fn check_for_updates_follows_settings_and_is_hidden_when_updates_are_disabled() {
    let entries = menu(VERSION, &inputs());
    let shown = labels(&entries);
    let settings = shown.iter().position(|label| label == "Settings…").unwrap();
    assert_eq!(shown[settings + 1], "Check for updates…");
    assert_eq!(
        item(&entries, TrayAction::CheckForUpdates),
        Some(("Check for updates…".into(), true))
    );

    let mut disabled = inputs();
    disabled.updates_enabled = false;
    assert_eq!(
        item(&menu(VERSION, &disabled), TrayAction::CheckForUpdates),
        None
    );

    let mut polish = inputs();
    polish.language = UiLanguage::Pl;
    assert_eq!(
        item(&menu(VERSION, &polish), TrayAction::CheckForUpdates)
            .unwrap()
            .0,
        "Sprawdź aktualizacje…"
    );
}

#[test]
fn check_for_updates_opens_the_app_page_and_runs_a_manual_check() {
    let rig = rig(FakeEngine::returning("x"), finite_source());
    let history = history(&[]);
    let target = Target::new(&rig.dictation, &history);

    perform(TrayAction::CheckForUpdates, &target);

    assert_eq!(*target.shown.borrow(), [MainPage::App]);
    assert_eq!(*target.update_checks.borrow(), 1);
}
