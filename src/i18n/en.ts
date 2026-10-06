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
      upcoming: "Cancel Shortcut and Microphone selection.",
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
      upcoming: "Start with Windows, recording indicator, automatic updates and the log folder.",
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
  },
  recordShortcut: {
    label: "Record Shortcut",
    description: "Starts and stops a Recording from any application.",
    change: "Change the Record Shortcut, currently {{shortcut}}",
    capturing: "Press the new shortcut…",
    captureHint: "Esc or a click elsewhere cancels.",
    reset: "Reset to default",
    keepsPrevious: "Your previous shortcut stays active.",
    problems: {
      empty: "Press at least one key.",
      needsModifier:
        "{{shortcut}} alone would stop working for typing. Add Ctrl, Alt, Shift or Win.",
      singleModifier:
        "A single modifier is too easy to press by accident. Use two modifiers, a modifier with a key, or right Alt or right Ctrl alone.",
      escapeReserved: "Esc is reserved for cancelling a Dictation.",
      reservedByWindows:
        "Windows keeps {{shortcut}} for itself and never passes it to applications.",
      sameAsCancel: "{{shortcut}} is already the Cancel Shortcut.",
      invalid: "{{shortcut}} cannot be used as a shortcut.",
    },
    activationFailed: "Echo could not activate {{shortcut}}: {{reason}}",
    captureUnavailable: "Changing the shortcut is not available right now.",
    mode: {
      label: "Shortcut mode",
      pushToTalk: "Hold to record",
      pushToTalkHint: "The Recording lasts exactly as long as you hold the shortcut.",
      toggle: "Press to start, press again to stop",
      toggleHint: "One press starts the Recording, the next press stops it.",
    },
  },
  devIntent: {
    badge: "Developer check",
    note: "Temporary: removed when the dictation pipeline (#13) arrives.",
    label: "Last recording intent",
    none: "none yet",
    start: "start",
    stop: "stop",
  },
  keys: {
    Space: "Space",
    Escape: "Esc",
    RightAlt: "Right Alt",
    RightCtrl: "Right Ctrl",
    ScrollLock: "Scroll Lock",
    PageUp: "Page Up",
    PageDown: "Page Down",
    PrintScreen: "Print Screen",
    CapsLock: "Caps Lock",
    NumLock: "Num Lock",
    ContextMenu: "Menu",
    Backspace: "Backspace",
    Up: "↑",
    Down: "↓",
    Left: "←",
    Right: "→",
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
