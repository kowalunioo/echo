import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { useTestAudio } from "../store/testAudio";

/**
 * The persistent "test audio" marker (rule 41), so a build in fake-microphone mode is never
 * mistaken for normal operation. The full form is a strip at the top of the main window and also
 * marks the window title; `compact` is a small badge for the Overlay.
 */
export function TestAudioMarker({ compact = false }: { compact?: boolean }) {
  const { t } = useTranslation();
  const file = useTestAudio();

  useEffect(() => {
    if (compact || file === null) return;
    getCurrentWindow()
      .setTitle(t("testAudio.windowTitle", { file }))
      .catch((error: unknown) => {
        console.error("cannot mark the window title", error);
      });
  }, [compact, file, t]);

  if (file === null) return null;
  if (compact) {
    return (
      <span
        role="status"
        aria-label={t("testAudio.title")}
        title={t("testAudio.description", { file })}
        className="rounded-full bg-accent-soft px-2 py-0.5 text-[10px] font-semibold tracking-wide text-accent-soft-fg"
      >
        {t("testAudio.badge")}
      </span>
    );
  }
  return (
    <div
      role="status"
      aria-label={t("testAudio.title")}
      className="flex shrink-0 items-center gap-3 border-b border-accent/40 bg-accent-soft px-4 py-2 text-sm text-accent-soft-fg"
    >
      <span className="rounded-full bg-accent-strong px-2 py-0.5 text-xs font-semibold tracking-wide text-accent-fg">
        {t("testAudio.badge")}
      </span>
      <span className="min-w-0 truncate">{t("testAudio.description", { file })}</span>
    </div>
  );
}
