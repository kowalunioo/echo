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
      upcoming: "Skrót nagrywania i jego tryb, skrót anulowania oraz wybór mikrofonu.",
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
      upcoming: "Uruchamianie z systemem Windows, wskaźnik nagrywania i automatyczne aktualizacje.",
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
    logFolder: {
      label: "Dziennik diagnostyczny",
      description:
        "Zdarzenia i błędy zapisywane na tym komputerze, pomocne przy rozwiązywaniu problemów. Nigdy nie zawiera tego, co dyktujesz.",
      open: "Otwórz folder logów",
      failed: "Nie udało się otworzyć folderu logów.",
    },
  },
  onboarding: {
    progress: "Kroki konfiguracji",
    steps: {
      welcome: "Powitanie",
      microphone: "Mikrofon",
      model: "Model",
      tryIt: "Wypróbuj",
    },
    welcome: {
      title: "Witamy w Echo",
      lead: "Mów, a Echo wpisze Twoje słowa w aplikacji, której używasz.",
      local: "Wszystko działa na tym komputerze. Twój głos i tekst nigdy go nie opuszczają.",
      model: "Echo potrzebuje modelu mowy: jednorazowe pobranie kilkuset MB.",
      shortcut: "Potem przytrzymaj skrót, mów i puść. Tekst pojawi się tam, gdzie piszesz.",
      start: "Zaczynamy",
    },
    microphone: {
      title: "Zezwól na dostęp do mikrofonu",
      lead: "Windows blokuje aplikacjom klasycznym dostęp do mikrofonu, więc Echo Cię nie słyszy.",
      howTo:
        "Otwórz ustawienia prywatności Windows i włącz dostęp dla aplikacji klasycznych. Echo przejdzie dalej samo, gdy tylko dostęp zostanie przyznany.",
      open: "Otwórz ustawienia prywatności Windows",
      checking: "Sprawdzam ponownie co kilka sekund…",
      skip: "Pomiń na razie",
      skipNote: "Nagrania nie będą działać, dopóki dostęp nie zostanie przyznany.",
    },
    model: {
      title: "Wybierz model",
      lead: "Model zamienia Twoją mowę na tekst. Pobierasz go raz i zostaje na tym komputerze.",
      upcoming:
        "Trzy modele z rozmiarami, zalecany Whisper large-v3-turbo oraz pobieranie z postępem.",
      continueWithout: "Kontynuuj bez modelu",
    },
    tryIt: {
      title: "Wypróbuj",
      lead: "Echo jest gotowe. Kliknij pole poniżej i podyktuj zdanie.",
      holdShortcut: "Przytrzymaj <kbd>{{shortcut}}</kbd> i mów, potem puść.",
      pressShortcut: "Naciśnij <kbd>{{shortcut}}</kbd> i mów, potem naciśnij ponownie.",
      fieldLabel: "Pole testowe",
      fieldPlaceholder: "Tu pojawią się Twoje słowa…",
      later: "Skrót możesz później zmienić w sekcji Dyktowanie.",
      finish: "Zakończ",
    },
  },
  history: {
    limit: {
      label: "Limit historii",
      decrease: "Przechowuj mniej transkrypcji",
      increase: "Przechowuj więcej transkrypcji",
      keeps_zero: "Nic nie jest przechowywane: nowe transkrypcje nie są zapisywane",
      keeps_one: "Przechowuje ostatnią transkrypcję",
      keeps_few: "Przechowuje ostatnie {{count}} transkrypcje",
      keeps_many: "Przechowuje ostatnie {{count}} transkrypcji",
      keeps_other: "Przechowuje ostatnie {{count}} transkrypcji",
    },
    list: "Transkrypcje",
    empty: "Nie ma jeszcze transkrypcji. Naciśnij <kbd>{{shortcut}}</kbd> i mów.",
    off: "Historia jest wyłączona. Zwiększ limit, aby zachowywać transkrypcje.",
    loadFailed: "Nie udało się wczytać historii.",
    copy: "Kopiuj",
    reinsert: "Wstaw ponownie",
    delete: "Usuń",
    showMore: "Pokaż więcej",
    showLess: "Pokaż mniej",
    copied: "Skopiowano",
    copyFailed: "Nie udało się skopiować tekstu.",
    deleted: "Usunięto transkrypcję",
    undo: "Cofnij",
    reinsertFailed: "Nie udało się wstawić tekstu.",
    clearAll: {
      button: "Wyczyść wszystko",
      title: "Usunąć wszystkie transkrypcje?",
      body: "Wszystkie transkrypcje z historii zostaną trwale usunięte. Tej operacji nie można cofnąć.",
      confirm: "Usuń wszystko",
      cancel: "Anuluj",
    },
  },
  errors: {
    settingsUnavailable: "Echo nie może wczytać ustawień. Uruchom Echo ponownie.",
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
