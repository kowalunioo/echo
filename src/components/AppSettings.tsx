import { type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";

import { commands } from "../bindings";
import { useShell } from "../store/shell";
import { AutostartSettings } from "./AutostartSettings";
import { OverlaySettings } from "./OverlaySettings";
import { UiLanguagePicker } from "./UiLanguagePicker";
import { UpdateSettings } from "./UpdateSettings";

export function AppSettings() {
  const { t } = useTranslation();
  const appInfo = useShell((s) => s.appInfo);

  const version =
    appInfo.status === "ready"
      ? appInfo.info.version
      : t(
          appInfo.status === "loading"
            ? "settings.version.loading"
            : "settings.version.unavailable",
        );
  const systemLanguage =
    appInfo.status === "ready" && appInfo.info.systemLocale
      ? appInfo.info.systemLocale
      : t("settings.systemLanguage.unknown");

  return (
    <section className="divide-y divide-line rounded-card border border-line bg-surface">
      <Row
        label={t("settings.uiLanguage.label")}
        description={t("settings.uiLanguage.description")}
      >
        <UiLanguagePicker />
      </Row>
      <AutostartSettings />
      <OverlaySettings />
      <Row label={t("settings.systemLanguage.label")}>
        <span className="text-muted">{systemLanguage}</span>
      </Row>
      <UpdateSettings version={version} />
      <Row label={t("settings.logFolder.label")} description={t("settings.logFolder.description")}>
        <OpenLogFolder />
      </Row>
    </section>
  );
}

export function Row({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-8 px-6 py-4">
      <div className="flex flex-col gap-0.5">
        <span className="font-medium">{label}</span>
        {description && <span className="text-xs text-muted">{description}</span>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function OpenLogFolder() {
  const { t } = useTranslation();
  const [failed, setFailed] = useState(false);

  const open = async () => {
    const result = await commands.openLogFolder();
    setFailed(result.status === "error");
  };

  return (
    <div className="flex flex-col items-end gap-1">
      <button
        type="button"
        onClick={() => void open()}
        className="rounded-md text-sm font-medium text-accent-strong underline-offset-4 hover:underline"
      >
        {t("settings.logFolder.open")}
      </button>
      {failed && (
        <span role="alert" className="text-xs text-muted">
          {t("settings.logFolder.failed")}
        </span>
      )}
    </div>
  );
}
