import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import type { UpdateStatus } from "../bindings";
import { useSetting } from "../store/settings";
import { useUpdater } from "../store/updater";
import { Row } from "./AppSettings";

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

  return (
    <>
      <Row
        label={t("settings.updates.automatic")}
        description={t("settings.updates.automaticDescription")}
      >
        <div className="flex flex-col items-end gap-1">
          <button
            type="button"
            role="switch"
            aria-checked={on}
            aria-label={t("settings.updates.automatic")}
            disabled={managed}
            onClick={() => void setAutomatic(!automatic)}
            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${
              on ? "bg-accent-strong" : "border border-line bg-raised"
            }`}
          >
            <span
              aria-hidden
              className={`inline-block size-4 rounded-full shadow transition-transform ${
                on ? "translate-x-6 bg-accent-fg" : "translate-x-1 bg-muted"
              }`}
            />
          </button>
          {managed && <span className="text-xs text-muted">{t("settings.updates.managed")}</span>}
        </div>
      </Row>
      <Row label={t("settings.version.label")}>
        <div className="flex flex-col items-end gap-1">
          <div className="flex items-center gap-4">
            <span className="text-muted tabular-nums" data-testid="app-version">
              {version}
            </span>
            {!managed && view !== null && (
              <button
                type="button"
                disabled={busy}
                onClick={() => void check()}
                className="rounded-md text-sm font-medium text-accent-strong underline-offset-4 hover:underline disabled:cursor-not-allowed disabled:opacity-50 disabled:no-underline"
              >
                {t("settings.updates.check")}
              </button>
            )}
          </div>
          {view !== null && !managed && <StatusLine status={view.status} />}
        </div>
      </Row>
    </>
  );
}

function StatusLine({ status }: { status: UpdateStatus }) {
  const { t } = useTranslation();
  const install = useUpdater((s) => s.install);

  const installButton = (
    <button
      type="button"
      onClick={() => void install()}
      className="rounded-md bg-accent-strong px-3 py-1 text-xs font-medium text-accent-fg hover:opacity-90"
    >
      {t("settings.updates.install")}
    </button>
  );

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
  const offersInstall = status.state === "available" || status.state === "ready";

  return (
    <div className="flex max-w-xs flex-col items-end gap-2 text-right">
      <span
        role="status"
        data-testid="update-status"
        className={`text-xs ${failed ? "text-danger" : "text-muted"}`}
      >
        {text}
      </span>
      {offersInstall && installButton}
    </div>
  );
}
