import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { commands } from "../bindings";
import { useSetting } from "../store/settings";
import { Button } from "./Button";
import { ExternalIcon } from "./icons";
import { SettingRow } from "./SettingRow";
import { Switch } from "./Switch";

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
    // While Windows has Echo turned off, its hint replaces the description.
    <SettingRow
      label={t("settings.autostart.label")}
      description={t(
        disabledInWindows
          ? "settings.autostart.disabledInWindows"
          : "settings.autostart.description",
      )}
      below={
        openFailed && (
          <span role="alert" className="text-note text-danger">
            {t("settings.autostart.openFailed")}
          </span>
        )
      }
    >
      {disabledInWindows && (
        <Button
          variant="quiet"
          onClick={() => void openStartupApps()}
          className="inline-flex items-center gap-1.5"
        >
          <ExternalIcon />
          {t("settings.autostart.openStartupApps")}
        </Button>
      )}
      <Switch
        checked={effective}
        label={t("settings.autostart.label")}
        disabled={disabledInWindows}
        onChange={() => void setEnabled(!enabled)}
      />
    </SettingRow>
  );
}
