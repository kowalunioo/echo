import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { commands, events } from "../bindings";

/**
 * The one-time hint shown the first time the main window is closed (tray.md rule 13). The
 * backend keeps the window open and asks for the hint; confirming it hides the window to the
 * tray and the hint is never asked for again.
 */
export function TrayHint() {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    events.trayHintRequested
      .listen(() => {
        setOpen(true);
      })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch((error: unknown) => {
        console.error("cannot listen for the tray hint", error);
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (open) button.current?.focus();
  }, [open]);

  if (!open) return null;

  const confirm = () => {
    setOpen(false);
    commands.closeToTray().catch((error: unknown) => {
      console.error("close_to_tray failed", error);
    });
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-6">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="tray-hint-title"
        aria-describedby="tray-hint-message"
        onKeyDown={(event) => {
          if (event.key === "Escape") confirm();
        }}
        className="flex max-w-sm flex-col gap-3 rounded-card border border-line bg-surface px-6 py-5 shadow-lg"
      >
        <h2 id="tray-hint-title" className="font-medium">
          {t("trayHint.title")}
        </h2>
        <p id="tray-hint-message" className="text-sm text-muted">
          {t("trayHint.message")}
        </p>
        <div className="flex justify-end">
          <button
            ref={button}
            type="button"
            onClick={confirm}
            className="rounded-lg bg-accent-strong px-4 py-1.5 font-medium text-accent-fg transition-opacity duration-150 hover:opacity-90 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus"
          >
            {t("trayHint.ok")}
          </button>
        </div>
      </div>
    </div>
  );
}
