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
  status: {
    label: "Stan",
    noModel: "Brak modelu",
  },
  languages: {
    pl: "Polski",
    en: "English",
  },
};
