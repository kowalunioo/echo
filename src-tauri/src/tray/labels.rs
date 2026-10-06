//! The tray's texts in both UI Languages (rule 11). The tray is drawn by the backend, so these
//! live here rather than in the frontend's i18n files.

use crate::dictation::ProblemKind;
use crate::settings::UiLanguage;

/// Every text the tray menu and tooltip use.
pub struct Labels {
    pub recording: &'static str,
    pub transcribing: &'static str,
    pub cancel: &'static str,
    pub copy_last_transcript: &'static str,
    pub model: &'static str,
    pub no_model_downloaded: &'static str,
    pub settings: &'static str,
    pub quit: &'static str,
    problems: [&'static str; 9],
}

impl Labels {
    pub fn for_language(language: UiLanguage) -> &'static Labels {
        match language {
            UiLanguage::En => &EN,
            UiLanguage::Pl => &PL,
        }
    }

    /// A short name for a Dictation error, for the tooltip.
    pub fn problem(&self, kind: ProblemKind) -> &'static str {
        let index = match kind {
            ProblemKind::NoModel => 0,
            ProblemKind::MicrophoneNotFound => 1,
            ProblemKind::MicrophoneAccessDenied => 2,
            ProblemKind::MicrophoneDisconnected => 3,
            ProblemKind::MicrophoneFailed => 4,
            ProblemKind::ModelLoadFailed => 5,
            ProblemKind::ModelDownloadFailed => 6,
            ProblemKind::TranscriptionFailed => 7,
            ProblemKind::InsertionFailed => 8,
        };
        self.problems[index]
    }
}

static EN: Labels = Labels {
    recording: "recording",
    transcribing: "transcribing",
    cancel: "Cancel",
    copy_last_transcript: "Copy last Transcript",
    model: "Model",
    no_model_downloaded: "No Model downloaded",
    settings: "Settings…",
    quit: "Quit Echo",
    problems: [
        "No Model",
        "No microphone found",
        "Microphone access is blocked",
        "Microphone disconnected",
        "The microphone could not be used",
        "The Model could not be loaded",
        "Model download failed",
        "Transcription failed",
        "Couldn't insert the text",
    ],
};

static PL: Labels = Labels {
    recording: "nagrywanie",
    transcribing: "transkrypcja",
    cancel: "Anuluj",
    copy_last_transcript: "Kopiuj ostatnią transkrypcję",
    model: "Model",
    no_model_downloaded: "Brak pobranego Modelu",
    settings: "Ustawienia…",
    quit: "Zakończ Echo",
    problems: [
        "Brak Modelu",
        "Nie znaleziono mikrofonu",
        "Dostęp do mikrofonu jest zablokowany",
        "Mikrofon odłączony",
        "Nie udało się użyć mikrofonu",
        "Nie udało się wczytać Modelu",
        "Pobieranie Modelu nie powiodło się",
        "Transkrypcja nie powiodła się",
        "Nie udało się wstawić tekstu",
    ],
};
