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
      upcoming: "Record Shortcut and its mode, Cancel Shortcut, and Microphone selection.",
    },
    model: {
      title: "Model & language",
      description: "The speech Model that turns your voice into text, and the language you speak.",
      upcoming: "The Dictation Language.",
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
      choose: "Models",
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
  models: {
    listLabel: "Models",
    recommended: "Recommended",
    active: "Active",
    languages: "{{count}} languages",
    descriptions: {
      whisperLargeV3Turbo: "Best accuracy; slower without a graphics card.",
      parakeetTdt06bV3:
        "Fast and accurate in 25 European languages; always detects the language itself.",
      whisperSmall: "Small and fast; lower accuracy.",
    },
    languagesOf: {
      whisperLargeV3Turbo: "100 languages",
      parakeetTdt06bV3: "25 European languages",
      whisperSmall: "99 languages",
    },
    size: "{{size}} MB",
    actions: {
      download: "Download ({{size}} MB)",
      cancel: "Cancel",
      resume: "Resume",
      retry: "Retry",
      use: "Use this Model",
      delete: "Delete",
    },
    state: {
      queued: "Waiting for the current download…",
      downloading: "{{percent}}% · {{speed}} MB/s",
      verifying: "Verifying…",
      paused: "Paused — {{percent}}% downloaded",
      loading: "Loading…",
      downloaded: "Downloaded",
    },
    progress: "Download progress of {{model}}",
    failure: {
      network: "The download failed: Echo could not reach the server.",
      stalled: "The download stopped: no data arrived for a minute.",
      badRange:
        "The server could not continue the download. Retry to download it from the beginning.",
      sizeMismatch: "The server sent a file of the wrong size.",
      corrupted: "Download was corrupted — please try again",
      storage: "The Model file could not be saved on this computer.",
      diskSpace: "Not enough disk space: {{needed}} MB of free space is needed.",
    },
    loadFailed: "Couldn't load {{model}}: {{reason}}",
    busy: "You can switch or delete Models once the Dictation has ended.",
    confirmDelete: {
      title: "Delete {{model}}?",
      body: "This frees {{size}} MB on this computer. You can download the Model again later.",
      confirm: "Delete",
      cancel: "Keep",
    },
    unload: {
      label: "Unload Model after inactivity",
      description:
        "Frees memory when you haven't dictated for a while. The next Dictation then takes a few seconds longer to start.",
      never: "Never",
      minutes: "{{count}} minutes",
    },
    indicator: {
      label: "Model",
      none: "Download a Model to start",
      ready: "Ready",
      loading: "Loading…",
      unloaded: "Unloaded (loads on next Dictation)",
      error: "Couldn't load",
      downloading: "Downloading {{percent}}%",
      open: "Open Model settings",
    },
    start: {
      title: "Download a Model to start",
      body: "Echo needs a speech Model before you can dictate. It is downloaded once and stays on this computer.",
      action: "Choose a Model",
    },
  },
  errors: {
    settingsUnavailable: "Echo could not load its settings. Please restart Echo.",
  },
  status: {
    label: "Status",
  },
  languages: {
    pl: "Polski",
    en: "English",
  },
};

export type Translation = typeof en;
