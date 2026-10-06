import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { events } from "./bindings";
import { DictationNotices } from "./components/DictationNotices";
import { PageView } from "./components/PageView";
import { Sidebar } from "./components/Sidebar";
import { changeUiLanguage } from "./i18n";
import { Onboarding } from "./onboarding/Onboarding";
import { useOwnWindowKeys } from "./shortcut/useOwnWindowKeys";
import { useSettings } from "./store/settings";
import { useShell } from "./store/shell";

export function App() {
  const loadAppInfo = useShell((s) => s.loadAppInfo);
  const loadSettings = useSettings((s) => s.load);
  const status = useSettings((s) => s.status);
  const uiLanguage = useSettings((s) => s.settings?.uiLanguage);
  const onboardingCompleted = useSettings((s) => s.settings?.onboardingCompleted);
  useOwnWindowKeys();

  useEffect(() => {
    void loadAppInfo();
    void loadSettings();
  }, [loadAppInfo, loadSettings]);

  // The UI Language applies the moment it changes, wherever it was changed (rule 8).
  useEffect(() => {
    if (uiLanguage) void changeUiLanguage(uiLanguage);
  }, [uiLanguage]);

  if (status === "loading") return null;
  if (status === "error") return <SettingsUnavailable />;
  return onboardingCompleted ? <MainWindow /> : <Onboarding />;
}

function MainWindow() {
  const page = useShell((s) => s.page);
  const setPage = useShell((s) => s.setPage);

  // Another surface (e.g. a clicked Overlay message) asks for a page.
  useEffect(() => {
    const stop = events.mainPageRequested.listen((event) => {
      setPage(event.payload);
    });
    return () => {
      void stop.then((unlisten) => {
        unlisten();
      });
    };
  }, [setPage]);

  return (
    <div className="flex h-full">
      <Sidebar />
      <main className="min-w-0 flex-1 overflow-y-auto">
        <DictationNotices />
        <PageView page={page} />
      </main>
    </div>
  );
}

function SettingsUnavailable() {
  const { t } = useTranslation();
  return (
    <div role="alert" className="flex h-full items-center justify-center p-10 text-muted">
      {t("errors.settingsUnavailable")}
    </div>
  );
}
