import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { useUpdater } from "../store/updater";

/** How long "Echo was updated to <version>" stays once the user can see it. */
export const UPDATED_NOTICE_MS = 8000;

/**
 * "Echo was updated to <version>" after an automatic update's restart (updater.md rule 6). It
 * waits while the window is hidden in the tray, then shows briefly once the window is opened.
 */
export function UpdatedNotice() {
  const { t } = useTranslation();
  const load = useUpdater((s) => s.load);
  const updatedTo = useUpdater((s) => s.view?.updatedTo ?? null);
  const dismiss = useUpdater((s) => s.dismissNotice);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (updatedTo === null) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const start = () => {
      if (timer === undefined && document.visibilityState === "visible") {
        timer = setTimeout(() => void dismiss(), UPDATED_NOTICE_MS);
      }
    };
    start();
    document.addEventListener("visibilitychange", start);
    window.addEventListener("focus", start);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("visibilitychange", start);
      window.removeEventListener("focus", start);
    };
  }, [updatedTo, dismiss]);

  if (updatedTo === null) return null;
  return (
    <section
      role="status"
      className="mx-auto mt-6 flex max-w-2xl items-center justify-between gap-4 rounded-card border border-line bg-accent-soft px-6 py-3 text-accent-soft-fg"
    >
      <span className="font-medium">{t("updatedNotice.text", { version: updatedTo })}</span>
      <button
        type="button"
        onClick={() => void dismiss()}
        className="rounded-md text-sm font-medium underline-offset-4 hover:underline"
      >
        {t("updatedNotice.dismiss")}
      </button>
    </section>
  );
}
