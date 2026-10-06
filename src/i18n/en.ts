/**
 * English UI strings. This object defines the shape every other language must match: the Polish
 * file is typed against it, so a missing or extra key fails `tsc`, and a test checks it again.
 * Use the terms from CONTEXT.md exactly (Dictation, Recording, Transcript, Record Shortcut, …).
 */
export const en = {
  app: {
    name: "Echo",
    tagline: "Private dictation",
  },
  nav: {
    label: "Sections",
    dictation: "Dictation",
    model: "Model & language",
    vocabulary: "Vocabulary",
    history: "History",
    app: "App",
  },
  pages: {
    dictation: {
      title: "Dictation",
      description:
        "How you start, stop and cancel a Dictation, and which Microphone Echo listens to.",
      upcoming: "Record Shortcut and its mode, and the Cancel Shortcut.",
    },
    model: {
      title: "Model & language",
      description: "The speech Model that turns your voice into text, and the language you speak.",
      upcoming: "Downloading and choosing a Model, and the Dictation Language.",
    },
    vocabulary: {
      title: "Vocabulary",
      description: "Names and terms Echo should recognise and spell exactly as you write them.",
      upcoming: "Your list of words and phrases, and how much of the hint budget they use.",
    },
    history: {
      title: "History",
      description: "Your most recent Transcripts, stored only on this computer.",
      upcoming: "Recent Transcripts with copy, insert again and delete, and the History limit.",
    },
    app: {
      title: "App",
      description: "Language of the interface, start-up, updates and information about Echo.",
      upcoming: "Start with Windows, the recording indicator and automatic updates.",
    },
  },
  placeholder: {
    badge: "Coming soon",
    lead: "This section is not ready yet. It will contain:",
  },
  settings: {
    uiLanguage: {
      label: "Interface language",
      description:
        "The language of Echo's own windows and menus. It does not change the language you dictate in.",
    },
    version: {
      label: "Version",
      loading: "Checking…",
      unavailable: "Unavailable",
    },
    systemLanguage: {
      label: "Windows display language",
      unknown: "Not reported",
    },
    logFolder: {
      label: "Diagnostic log",
      description:
        "Events and errors, kept on this computer to help solve problems. It never contains what you dictate.",
      open: "Open log folder",
      failed: "The log folder could not be opened.",
    },
    microphone: {
      label: "Microphone",
      description:
        "The input device Echo listens to while you dictate. A change applies from the next Recording.",
      defaultOption: "Default ({{name}})",
      defaultUnknown: "Default",
      notConnected: "{{name}} (not connected)",
      loading: "Looking for microphones…",
      listFailed: "The list of microphones could not be loaded.",
      none: "No microphone found. Connect one to dictate.",
      reset: "Use default",
    },
  },
  onboarding: {
    progress: "Setup steps",
    steps: {
      welcome: "Welcome",
      microphone: "Microphone",
      model: "Model",
      tryIt: "Try it",
    },
    welcome: {
      title: "Welcome to Echo",
      lead: "Speak, and Echo types what you say into the app you are using.",
      local: "Everything runs on this computer. Your voice and your text never leave it.",
      model: "Echo needs a speech Model: a one-time download of a few hundred MB.",
      shortcut: "Then hold a shortcut, speak, and let go. The text appears where you are typing.",
      start: "Get started",
    },
    microphone: {
      title: "Allow microphone access",
      lead: "Windows is blocking desktop apps from using the microphone, so Echo cannot hear you.",
      howTo:
        "Open the Windows privacy settings and turn on access for desktop apps. Echo continues on its own as soon as access is allowed.",
      open: "Open Windows privacy settings",
      checking: "Checking again every few seconds…",
      skip: "Skip for now",
      skipNote: "Recordings will fail until access is allowed.",
    },
    model: {
      title: "Choose a Model",
      lead: "The Model turns your speech into text. It is downloaded once and stays on this computer.",
      upcoming:
        "The three Models with their sizes, Whisper large-v3-turbo recommended, and the download with its progress.",
      continueWithout: "Continue without a Model",
    },
    tryIt: {
      title: "Try it",
      lead: "Echo is ready. Click the field below and dictate a sentence.",
      holdShortcut: "Hold <kbd>{{shortcut}}</kbd> and speak, then let go.",
      pressShortcut: "Press <kbd>{{shortcut}}</kbd> and speak, then press it again.",
      fieldLabel: "Test field",
      fieldPlaceholder: "Your words appear here…",
      later: "You can change the shortcut later under Dictation.",
      finish: "Finish",
    },
  },
  errors: {
    settingsUnavailable: "Echo could not load its settings. Please restart Echo.",
  },
  /** Microphone messages for the Overlay and error notices (microphone.md, dictation-pipeline.md rule 8). */
  microphone: {
    fallback: "Selected microphone not found — using the default microphone",
    notFound: "No microphone found",
    accessDenied: "Microphone access is blocked in Windows privacy settings",
    disconnected: "Microphone disconnected — recording stopped",
    failed: "The microphone could not be opened: {{detail}}",
    openPrivacySettings: "Open Windows privacy settings",
  },
  status: {
    label: "Status",
    noModel: "No Model yet",
  },
  languages: {
    pl: "Polski",
    en: "English",
  },
};

export type Translation = typeof en;
