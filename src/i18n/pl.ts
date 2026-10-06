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
      upcoming: "Język dyktowania.",
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
      choose: "Modele",
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
  models: {
    listLabel: "Modele",
    recommended: "Polecany",
    active: "Aktywny",
    languages: "{{count}} języków",
    descriptions: {
      whisperLargeV3Turbo: "Najlepsza dokładność; wolniejszy bez karty graficznej.",
      parakeetTdt06bV3:
        "Szybki i dokładny w 25 językach europejskich; zawsze sam rozpoznaje język.",
      whisperSmall: "Mały i szybki; mniejsza dokładność.",
    },
    languagesOf: {
      whisperLargeV3Turbo: "100 języków",
      parakeetTdt06bV3: "25 języków europejskich",
      whisperSmall: "99 języków",
    },
    size: "{{size}} MB",
    actions: {
      download: "Pobierz ({{size}} MB)",
      cancel: "Anuluj",
      resume: "Wznów",
      retry: "Spróbuj ponownie",
      use: "Używaj tego modelu",
      delete: "Usuń",
    },
    state: {
      queued: "Czeka na zakończenie bieżącego pobierania…",
      downloading: "{{percent}}% · {{speed}} MB/s",
      verifying: "Sprawdzanie…",
      paused: "Wstrzymano — pobrano {{percent}}%",
      loading: "Wczytywanie…",
      downloaded: "Pobrany",
    },
    progress: "Postęp pobierania: {{model}}",
    failure: {
      network: "Pobieranie nie powiodło się: Echo nie może połączyć się z serwerem.",
      stalled: "Pobieranie stanęło: przez minutę nie dotarły żadne dane.",
      badRange: "Serwer nie mógł wznowić pobierania. Spróbuj ponownie, aby pobrać od początku.",
      sizeMismatch: "Serwer wysłał plik o złym rozmiarze.",
      corrupted: "Pobrany plik jest uszkodzony — spróbuj ponownie",
      storage: "Nie udało się zapisać pliku modelu na tym komputerze.",
      diskSpace: "Za mało miejsca na dysku: potrzeba {{needed}} MB wolnego miejsca.",
    },
    loadFailed: "Nie udało się wczytać modelu {{model}}: {{reason}}",
    busy: "Model można zmienić lub usunąć po zakończeniu dyktowania.",
    confirmDelete: {
      title: "Usunąć model {{model}}?",
      body: "Zwolni to {{size}} MB na tym komputerze. Model można później pobrać ponownie.",
      confirm: "Usuń",
      cancel: "Zostaw",
    },
    unload: {
      label: "Zwalniaj model z pamięci po bezczynności",
      description:
        "Zwalnia pamięć, gdy przez jakiś czas nie dyktujesz. Następne dyktowanie rozpocznie się wtedy kilka sekund później.",
      never: "Nigdy",
      minutes: "{{count}} min",
    },
    indicator: {
      label: "Model",
      none: "Pobierz model, aby zacząć",
      ready: "Gotowy",
      loading: "Wczytywanie…",
      unloaded: "Zwolniony (wczyta się przy następnym dyktowaniu)",
      error: "Nie udało się wczytać",
      downloading: "Pobieranie {{percent}}%",
      open: "Otwórz ustawienia modelu",
    },
    start: {
      title: "Pobierz model, aby zacząć",
      body: "Do dyktowania Echo potrzebuje modelu mowy. Pobiera się go raz i zostaje na tym komputerze.",
      action: "Wybierz model",
    },
  },
  errors: {
    settingsUnavailable: "Echo nie może wczytać ustawień. Uruchom Echo ponownie.",
  },
  status: {
    label: "Stan",
  },
  languages: {
    pl: "Polski",
    en: "English",
  },
};
