import type { Translation } from "./en";

/** Polish UI strings; typed against the English file so both always have the same keys. */
export const pl: Translation = {
  app: {
    name: "Echo",
    tagline: "Prywatne dyktowanie",
  },
  nav: {
    label: "Sekcje",
    dictation: "Dyktowanie",
    model: "Model i język",
    vocabulary: "Słownik",
    history: "Historia",
    app: "Aplikacja",
  },
  pages: {
    dictation: {
      title: "Dyktowanie",
      description:
        "Jak rozpoczynasz, kończysz i anulujesz dyktowanie oraz którego mikrofonu słucha Echo.",
      upcoming: "Skrót anulowania oraz wybór mikrofonu.",
    },
    model: {
      title: "Model i język",
      description: "Model mowy, który zamienia Twój głos na tekst, oraz język, w którym mówisz.",
      upcoming: "Pobieranie i wybór modelu oraz język dyktowania.",
    },
    vocabulary: {
      title: "Słownik",
      description: "Nazwy i terminy, które Echo ma rozpoznawać i zapisywać dokładnie tak jak Ty.",
      upcoming: "Twoja lista słów i fraz oraz to, ile budżetu podpowiedzi zajmują.",
    },
    history: {
      title: "Historia",
      description: "Twoje ostatnie transkrypcje, przechowywane wyłącznie na tym komputerze.",
      upcoming:
        "Ostatnie transkrypcje z kopiowaniem, ponownym wstawieniem i usuwaniem oraz limit historii.",
    },
    app: {
      title: "Aplikacja",
      description: "Język interfejsu, uruchamianie, aktualizacje i informacje o Echo.",
      upcoming:
        "Uruchamianie z systemem Windows, wskaźnik nagrywania, automatyczne aktualizacje i folder logów.",
    },
  },
  placeholder: {
    badge: "Wkrótce",
    lead: "Ta sekcja nie jest jeszcze gotowa. Znajdzie się tu:",
  },
  settings: {
    uiLanguage: {
      label: "Język interfejsu",
      description: "Język okien i menu Echo. Nie zmienia języka, w którym dyktujesz.",
    },
    version: {
      label: "Wersja",
      loading: "Sprawdzanie…",
      unavailable: "Niedostępna",
    },
    systemLanguage: {
      label: "Język wyświetlania Windows",
      unknown: "Nieznany",
    },
  },
  recordShortcut: {
    label: "Skrót nagrywania",
    description: "Rozpoczyna i kończy nagrywanie w dowolnej aplikacji.",
    change: "Zmień skrót nagrywania, obecnie {{shortcut}}",
    capturing: "Naciśnij nowy skrót…",
    captureHint: "Esc lub kliknięcie obok anuluje.",
    reset: "Przywróć domyślny",
    keepsPrevious: "Poprzedni skrót pozostaje aktywny.",
    problems: {
      empty: "Naciśnij co najmniej jeden klawisz.",
      needsModifier:
        "Sam klawisz {{shortcut}} przestałby działać przy pisaniu. Dodaj Ctrl, Alt, Shift lub Win.",
      singleModifier:
        "Pojedynczy modyfikator łatwo nacisnąć przypadkiem. Użyj dwóch modyfikatorów, modyfikatora z klawiszem albo samego prawego Alt lub prawego Ctrl.",
      escapeReserved: "Klawisz Esc jest zarezerwowany do anulowania dyktowania.",
      reservedByWindows:
        "Windows zatrzymuje {{shortcut}} dla siebie i nie przekazuje go aplikacjom.",
      sameAsCancel: "{{shortcut}} jest już skrótem anulowania.",
      invalid: "{{shortcut}} nie może być skrótem.",
    },
    activationFailed: "Echo nie może włączyć skrótu {{shortcut}}: {{reason}}",
    captureUnavailable: "Zmiana skrótu jest teraz niedostępna.",
    mode: {
      label: "Tryb skrótu",
      pushToTalk: "Przytrzymaj, aby nagrywać",
      pushToTalkHint: "Nagrywanie trwa dokładnie tak długo, jak trzymasz skrót.",
      toggle: "Naciśnij, aby zacząć, naciśnij ponownie, aby zakończyć",
      toggleHint: "Pierwsze naciśnięcie rozpoczyna nagrywanie, kolejne je kończy.",
    },
  },
  devIntent: {
    badge: "Kontrola deweloperska",
    note: "Tymczasowe: zniknie, gdy powstanie potok dyktowania (#13).",
    label: "Ostatnie polecenie nagrywania",
    none: "jeszcze brak",
    start: "rozpocznij",
    stop: "zatrzymaj",
  },
  keys: {
    Space: "Spacja",
    Escape: "Esc",
    RightAlt: "Prawy Alt",
    RightCtrl: "Prawy Ctrl",
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
    label: "Stan",
    noModel: "Brak modelu",
  },
  languages: {
    pl: "Polski",
    en: "English",
  },
};
