import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { commands, events } from "../bindings";
import { Button } from "./Button";
import { Modal } from "./Dialog";

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

  if (!open) return null;

  const confirm = () => {
    setOpen(false);
    commands.closeToTray().catch((error: unknown) => {
      console.error("close_to_tray failed", error);
    });
  };

  return (
    <Modal
      labelledBy="tray-hint-title"
      describedBy="tray-hint-message"
      onClose={confirm}
      closeOnBackdrop={false}
      initialFocus={button}
    >
      <h2 id="tray-hint-title" className="text-heading">
        {t("trayHint.title")}
      </h2>
      <p id="tray-hint-message" className="text-muted">
        {t("trayHint.message")}
      </p>
      <div className="flex justify-end">
        <Button ref={button} onClick={confirm}>
          {t("trayHint.ok")}
        </Button>
      </div>
    </Modal>
  );
}
