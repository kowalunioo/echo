//! The tray (`docs/specs/tray.md`): Echo's icon in the Windows notification area, its tooltip
//! and its menu.
//!
//! - this module: what the tray shows, as pure logic — [`icon_state`], [`tooltip`], [`menu`] —
//!   and the [`TrayController`] that keeps a [`TrayView`] in step with its inputs;
//! - [`icons`]: the generated glyph images (`bun run tray-icons`);
//! - [`labels`]: the menu and tooltip texts in both UI Languages;
//! - [`actions`]: what the menu items do;
//! - [`theme`]: following the Windows taskbar theme;
//! - `app`: the Tauri wiring (the real tray icon, close-to-tray, the commands).

pub mod actions;
pub mod app;
pub mod icons;
pub mod labels;
pub mod theme;

#[cfg(windows)]
mod clipboard;

use crate::dictation::{DictationProblem, DictationState, DictationStatus, ProblemKind};
use crate::models::{ModelId, ModelsState};
use crate::settings::UiLanguage;

use labels::Labels;

/// What the tray icon shows (rule 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrayIconState {
    Idle,
    Recording,
    /// Transcribing or Inserting.
    Transcribing,
    /// The red error variant (rule 4a).
    Error,
}

/// The Windows taskbar theme. A light taskbar gets the dark glyph and vice versa (rule 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskbarTheme {
    Light,
    Dark,
}

/// The icon for a dictation status: a Recording or Transcribing Dictation wins over an earlier
/// error; otherwise an error shows red until it clears (rule 4a).
pub fn icon_state(status: &DictationStatus) -> TrayIconState {
    match status.state {
        DictationState::Recording => TrayIconState::Recording,
        DictationState::Transcribing => TrayIconState::Transcribing,
        _ if status.error.is_some() => TrayIconState::Error,
        DictationState::Inserting => TrayIconState::Transcribing,
        DictationState::Idle => TrayIconState::Idle,
    }
}

/// The hover text: "Echo <version>", plus the state when not idle or the error (rule 4).
pub fn tooltip(language: UiLanguage, version: &str, status: &DictationStatus) -> String {
    let labels = Labels::for_language(language);
    let base = format!("Echo {version}");
    let suffix = match icon_state(status) {
        TrayIconState::Idle => return base,
        TrayIconState::Recording => labels.recording.to_owned(),
        TrayIconState::Transcribing => labels.transcribing.to_owned(),
        TrayIconState::Error => match &status.error {
            Some(problem) => problem_text(labels, problem),
            None => return base,
        },
    };
    // Windows cuts tray tooltips at 127 characters.
    format!("{base} — {suffix}").chars().take(127).collect()
}

/// The error as the tooltip names it; Model problems also name the Model. Their detail is
/// "<Model name>: <technical detail>" (`dictation::app`).
fn problem_text(labels: &Labels, problem: &DictationProblem) -> String {
    let text = labels.problem(problem.kind);
    match problem.kind {
        ProblemKind::ModelDownloadFailed | ProblemKind::ModelLoadFailed => {
            match problem.detail.split_once(": ").map(|(name, _)| name.trim()) {
                Some(name) if !name.is_empty() => format!("{text} ({name})"),
                _ => text.to_owned(),
            }
        }
        _ => text.to_owned(),
    }
}

/// What a menu item does when chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    Cancel,
    CopyLastTranscript,
    ActivateModel(ModelId),
    Settings,
    Quit,
}

impl TrayAction {
    /// The menu item id the action is registered under.
    pub fn id(self) -> String {
        match self {
            TrayAction::Cancel => "cancel".into(),
            TrayAction::CopyLastTranscript => "copy-last-transcript".into(),
            TrayAction::ActivateModel(model) => format!("model-{}", model.index()),
            TrayAction::Settings => "settings".into(),
            TrayAction::Quit => "quit".into(),
        }
    }

    /// The action behind a menu item id.
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "cancel" => TrayAction::Cancel,
            "copy-last-transcript" => TrayAction::CopyLastTranscript,
            "settings" => TrayAction::Settings,
            "quit" => TrayAction::Quit,
            other => {
                let index: usize = other.strip_prefix("model-")?.parse().ok()?;
                TrayAction::ActivateModel(*ModelId::ALL.get(index)?)
            }
        })
    }
}

/// One entry of the tray menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    /// Disabled, informational text.
    Info(String),
    Separator,
    Item {
        action: TrayAction,
        label: String,
        enabled: bool,
    },
    /// The Model submenu (rule 7.5).
    Models {
        label: String,
        enabled: bool,
        choices: Vec<ModelChoice>,
    },
}

/// A downloaded Model in the Model submenu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelChoice {
    pub model: ModelId,
    pub name: String,
    pub active: bool,
}

/// The Models as the menu needs them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MenuModels {
    /// Downloaded Models in list order.
    pub downloaded: Vec<(ModelId, String)>,
    pub active: Option<ModelId>,
    /// Switching is not possible now (`models.md` rule 20).
    pub busy: bool,
}

impl MenuModels {
    pub fn from_state(state: &ModelsState) -> Self {
        Self {
            downloaded: state
                .models
                .iter()
                .filter(|m| m.downloaded)
                .map(|m| (m.id, m.name.clone()))
                .collect(),
            active: state.active,
            busy: state.dictation_in_progress,
        }
    }
}

/// Everything the tray shows depends on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayInputs {
    pub status: DictationStatus,
    pub theme: TaskbarTheme,
    pub language: UiLanguage,
    pub models: MenuModels,
    pub history_empty: bool,
}

/// The tray menu (rules 7–9, 11).
pub fn menu(version: &str, inputs: &TrayInputs) -> Vec<MenuEntry> {
    let labels = Labels::for_language(inputs.language);
    let busy = matches!(
        inputs.status.state,
        DictationState::Recording | DictationState::Transcribing
    );
    let item = |action, label: &str, enabled| MenuEntry::Item {
        action,
        label: label.to_owned(),
        enabled,
    };

    let mut entries = vec![
        MenuEntry::Info(format!("Echo {version}")),
        MenuEntry::Separator,
    ];
    if busy {
        entries.push(item(TrayAction::Cancel, labels.cancel, true));
        entries.push(MenuEntry::Separator);
    }
    entries.push(item(
        TrayAction::CopyLastTranscript,
        labels.copy_last_transcript,
        !inputs.history_empty,
    ));
    entries.push(MenuEntry::Separator);
    entries.push(models_entry(labels, &inputs.models, busy));
    entries.push(MenuEntry::Separator);
    entries.push(item(TrayAction::Settings, labels.settings, true));
    // "Check for updates…" (rule 7.8) goes here; it arrives with the updater (#24).
    entries.push(MenuEntry::Separator);
    entries.push(item(TrayAction::Quit, labels.quit, true));
    entries
}

fn models_entry(labels: &Labels, models: &MenuModels, dictating: bool) -> MenuEntry {
    if models.downloaded.is_empty() {
        return MenuEntry::Models {
            label: labels.no_model_downloaded.to_owned(),
            enabled: false,
            choices: Vec::new(),
        };
    }
    let active_name = models
        .downloaded
        .iter()
        .find(|(id, _)| Some(*id) == models.active)
        .map(|(_, name)| name.clone());
    MenuEntry::Models {
        label: active_name.unwrap_or_else(|| labels.model.to_owned()),
        enabled: !(dictating || models.busy),
        choices: models
            .downloaded
            .iter()
            .map(|(id, name)| ModelChoice {
                model: *id,
                name: name.clone(),
                active: Some(*id) == models.active,
            })
            .collect(),
    }
}

/// The real tray icon, or a fake in tests.
pub trait TrayView: Send {
    fn show_icon(&mut self, state: TrayIconState, theme: TaskbarTheme);
    fn show_tooltip(&mut self, text: &str);
    fn show_menu(&mut self, entries: &[MenuEntry]);
}

/// A change to one of the tray's inputs.
#[derive(Debug, Clone)]
pub enum TrayUpdate {
    Status(DictationStatus),
    Theme(TaskbarTheme),
    Language(UiLanguage),
    Models(MenuModels),
    HistoryEmpty(bool),
    /// Show the icon and the menu again: the display scale changed, or a click toggled a
    /// check mark on its own.
    Refresh,
}

/// Keeps a [`TrayView`] in step with the [`TrayInputs`], touching only what changed.
pub struct TrayController<V> {
    view: V,
    version: String,
    inputs: TrayInputs,
    icon: (TrayIconState, TaskbarTheme),
    tooltip: String,
    menu: Vec<MenuEntry>,
}

impl<V: TrayView> TrayController<V> {
    /// Shows everything for `inputs` on `view`.
    pub fn new(mut view: V, version: impl Into<String>, inputs: TrayInputs) -> Self {
        let version = version.into();
        let icon = (icon_state(&inputs.status), inputs.theme);
        let tooltip = tooltip(inputs.language, &version, &inputs.status);
        let menu = menu(&version, &inputs);
        view.show_icon(icon.0, icon.1);
        view.show_tooltip(&tooltip);
        view.show_menu(&menu);
        Self {
            view,
            version,
            inputs,
            icon,
            tooltip,
            menu,
        }
    }

    pub fn apply(&mut self, update: TrayUpdate) {
        match update {
            TrayUpdate::Status(status) => self.inputs.status = status,
            TrayUpdate::Theme(theme) => self.inputs.theme = theme,
            TrayUpdate::Language(language) => self.inputs.language = language,
            TrayUpdate::Models(models) => self.inputs.models = models,
            TrayUpdate::HistoryEmpty(empty) => self.inputs.history_empty = empty,
            TrayUpdate::Refresh => {
                self.view.show_icon(self.icon.0, self.icon.1);
                self.view.show_menu(&self.menu);
            }
        }
        let icon = (icon_state(&self.inputs.status), self.inputs.theme);
        if icon != self.icon {
            self.icon = icon;
            self.view.show_icon(icon.0, icon.1);
        }
        let tooltip = tooltip(self.inputs.language, &self.version, &self.inputs.status);
        if tooltip != self.tooltip {
            self.view.show_tooltip(&tooltip);
            self.tooltip = tooltip;
        }
        let menu = menu(&self.version, &self.inputs);
        if menu != self.menu {
            self.view.show_menu(&menu);
            self.menu = menu;
        }
    }

    pub fn view(&self) -> &V {
        &self.view
    }
}

#[cfg(test)]
mod tests;
