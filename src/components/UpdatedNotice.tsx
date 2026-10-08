import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { useUpdater } from "../store/updater";
import { IconButton } from "./IconButton";
import { CheckIcon, CloseIcon } from "./icons";

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
  // One quiet line at the top of the page column, closed off by a hairline.
  return (
    <section
      role="status"
      className="-mt-2 flex items-center justify-between gap-4 border-b border-line pb-2"
    >
      <span className="flex min-w-0 items-center gap-2">
        <span className="shrink-0 text-accent">
          <CheckIcon />
        </span>
        <span className="truncate">{t("updatedNotice.text", { version: updatedTo })}</span>
      </span>
      <IconButton
        label={t("updatedNotice.dismiss")}
        onClick={() => void dismiss()}
        className="-mr-1.5"
      >
        <CloseIcon />
      </IconButton>
    </section>
  );
}
