import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { commands } from "../bindings";
import { useSetting } from "../store/settings";
import { Row } from "./AppSettings";

/** Whether Windows "Startup apps" has turned Echo off; `false` when it cannot be told. */
async function loadDisabledInWindows(): Promise<boolean> {
  try {
    return (await commands.autostartStatus()).disabledInWindows;
  } catch (error) {
    console.error("autostart_status failed", error);
    return false;
  }
}

/**
 * "Start Echo when I sign in to Windows" (autostart.md "UI"). The backend registers or removes
 * the sign-in entry as soon as the setting changes. When Windows "Startup apps" has turned Echo
 * off, the toggle shows off, cannot be switched (Echo never re-enables itself, rule 9) and links
 * to that Windows page instead. The Windows state is checked again whenever the window regains
 * focus, so coming back from Windows settings shows the change.
 */
export function AutostartSettings() {
  const { t } = useTranslation();
  const [enabled, setEnabled] = useSetting("startWithWindows");
  const [disabledInWindows, setDisabledInWindows] = useState(false);
  const [openFailed, setOpenFailed] = useState(false);

  useEffect(() => {
    let live = true;
    const refresh = () => {
      void loadDisabledInWindows().then((disabled) => {
        if (live) setDisabledInWindows(disabled);
      });
    };
    refresh();
    window.addEventListener("focus", refresh);
    return () => {
      live = false;
      window.removeEventListener("focus", refresh);
    };
  }, [enabled]);

  const effective = enabled && !disabledInWindows;

  const openStartupApps = async () => {
    const result = await commands.openStartupAppsSettings();
    setOpenFailed(result.status === "error");
  };

  return (
    <Row label={t("settings.autostart.label")} description={t("settings.autostart.description")}>
      <div className="flex flex-col items-end gap-1">
        <button
          type="button"
          role="switch"
          aria-checked={effective}
          aria-label={t("settings.autostart.label")}
          disabled={disabledInWindows}
          onClick={() => void setEnabled(!enabled)}
          className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${
            effective ? "bg-accent-strong" : "border border-line bg-raised"
          }`}
        >
          <span
            aria-hidden
            className={`inline-block size-4 rounded-full shadow transition-transform ${
              effective ? "translate-x-6 bg-accent-fg" : "translate-x-1 bg-muted"
            }`}
          />
        </button>
        {disabledInWindows && (
          <>
            <span className="text-xs text-muted">{t("settings.autostart.disabledInWindows")}</span>
            <button
              type="button"
              onClick={() => void openStartupApps()}
              className="rounded-md text-xs font-medium text-accent-strong underline-offset-4 hover:underline"
            >
              {t("settings.autostart.openStartupApps")}
            </button>
          </>
        )}
        {openFailed && (
          <span role="alert" className="text-xs text-muted">
            {t("settings.autostart.openFailed")}
          </span>
        )}
      </div>
    </Row>
  );
}
