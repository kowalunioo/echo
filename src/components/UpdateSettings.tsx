import { useEffect } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";

import type { UpdateStatus } from "../bindings";
import { formatDate } from "../i18n";
import { ProgressBar } from "../models/ModelCard";
import { useSetting } from "../store/settings";
import { useUpdater } from "../store/updater";
import { Button } from "./Button";
import { RefreshIcon } from "./icons";
import { SettingRow } from "./SettingRow";
import { Switch } from "./Switch";

/**
 * The updates area of the App page (updater.md "UI"): the automatic-updates toggle, the current
 * version with "Check for updates", and the status line. When the environment disables the
 * updater (rule 11) the toggle is read-only with "Updates are managed by your system" and there
 * is no check button.
 */
export function UpdateSettings({ version }: { version: string }) {
  const { t } = useTranslation();
  const [automatic, setAutomatic] = useSetting("checkUpdatesAutomatically");
  const view = useUpdater((s) => s.view);
  const load = useUpdater((s) => s.load);
  const check = useUpdater((s) => s.check);

  useEffect(() => {
    void load();
  }, [load]);

  const managed = view?.managed ?? false;
  const on = automatic && !managed;
  const busy =
    view !== null && ["checking", "downloading", "installing"].includes(view.status.state);
  const offersInstall =
    view !== null && !managed && ["available", "ready"].includes(view.status.state);

  return (
    <>
      <SettingRow
        label={t("settings.updates.automatic")}
        description={t(
          managed ? "settings.updates.managed" : "settings.updates.automaticDescription",
        )}
      >
        <Switch
          checked={on}
          label={t("settings.updates.automatic")}
          disabled={managed}
          onChange={() => void setAutomatic(!automatic)}
        />
      </SettingRow>
      <SettingRow
        label={t("settings.version.label")}
        description={view?.lastChecked != null ? lastChecked(view.lastChecked, t) : undefined}
        tag={
          <span className="text-muted tabular-nums" data-testid="app-version">
            {version}
          </span>
        }
        below={view !== null && !managed && <StatusLine status={view.status} />}
      >
        {/* One action at a time: while an update is on offer, Install takes Check's place. */}
        {offersInstall && <InstallButton />}
        {!managed && view !== null && !offersInstall && (
          // Borderless: the text lines up with the row's edge, the hover fill reaches past it.
          <Button
            variant="quiet"
            disabled={busy}
            onClick={() => void check()}
            className="-mr-3.5 inline-flex items-center gap-1.5"
          >
            <RefreshIcon />
            {t("settings.updates.check")}
          </Button>
        )}
      </SettingRow>
    </>
  );
}

/** "Last checked: today at 14:02", or with the date when it was not today. */
function lastChecked(at: number, t: TFunction): string {
  const when = new Date(at);
  const today = new Date().toDateString() === when.toDateString();
  return t("settings.updates.lastChecked", {
    when: today
      ? t("settings.updates.today", { time: formatDate(when, { timeStyle: "short" }) })
      : formatDate(when),
  });
}

/** The one strong action of the page, so it is the white button. */
function InstallButton() {
  const { t } = useTranslation();
  const install = useUpdater((s) => s.install);
  return (
    <Button variant="contrast" onClick={() => void install()}>
      {t("settings.updates.install")}
    </Button>
  );
}

function StatusLine({ status }: { status: UpdateStatus }) {
  const { t } = useTranslation();

  const text = (() => {
    switch (status.state) {
      case "idle":
        return null;
      case "checking":
        return t("settings.updates.status.checking");
      case "upToDate":
        return t("settings.updates.status.upToDate");
      case "available":
        return t("settings.updates.status.available", { version: status.version });
      case "ready":
        return t("settings.updates.status.ready", { version: status.version });
      case "downloading":
        return status.percent === null
          ? t("settings.updates.status.downloadingUnknown")
          : t("settings.updates.status.downloading", { percent: status.percent });
      case "installing":
        return t("settings.updates.status.installing");
      case "checkFailed":
        return t("settings.updates.status.checkFailed");
      case "unverified":
        return t("settings.updates.status.unverified");
      case "downloadFailed":
        return t("settings.updates.status.downloadFailed");
      case "installFailed":
        return t("settings.updates.status.installFailed");
    }
  })();
  if (text === null) return null;

  const failed = ["checkFailed", "unverified", "downloadFailed", "installFailed"].includes(
    status.state,
  );

  // Under the row, so a long status (Polish "ready" runs two lines) wraps instead of being cut.
  return (
    <>
      <span
        role="status"
        data-testid="update-status"
        className={`text-note ${failed ? "text-danger" : "text-muted"}`}
      >
        {text}
      </span>
      {status.state === "downloading" && status.percent !== null && (
        <ProgressBar percent={status.percent} label={text} />
      )}
    </>
  );
}
