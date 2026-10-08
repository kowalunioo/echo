import { type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";

import { type UiLanguage, commands } from "../bindings";
import { useSetting } from "../store/settings";
import { useShell } from "../store/shell";
import { AutostartSettings } from "./AutostartSettings";
import { OverlaySettings } from "./OverlaySettings";
import { UiLanguagePicker } from "./UiLanguagePicker";
import { UpdateSettings } from "./UpdateSettings";

/**
 * A Windows locale such as "en-US" as a language name in the UI Language ("American English",
 * "angielski amerykański"), or the code itself when it has no name.
 */
function localeName(locale: string, uiLanguage: UiLanguage): string {
  try {
    const name = new Intl.DisplayNames([uiLanguage], { type: "language" }).of(locale);
    if (!name) return locale;
    return name.charAt(0).toLocaleUpperCase(uiLanguage) + name.slice(1);
  } catch {
    return locale;
  }
}

export function AppSettings() {
  const { t } = useTranslation();
  const appInfo = useShell((s) => s.appInfo);
  const [uiLanguage] = useSetting("uiLanguage");

  const version =
    appInfo.status === "ready"
      ? appInfo.info.version
      : t(
          appInfo.status === "loading"
            ? "settings.version.loading"
            : "settings.version.unavailable",
        );
  const systemLocale = appInfo.status === "ready" ? appInfo.info.systemLocale : null;

  return (
    <>
      <section className="divide-y divide-line rounded-card border border-line bg-surface">
        <Row
          label={t("settings.uiLanguage.label")}
          description={t("settings.uiLanguage.description")}
        >
          <UiLanguagePicker />
        </Row>
        <AutostartSettings />
        <OverlaySettings />
      </section>
      {/* Facts and upkeep, not choices: no fill, so the settings above lead. */}
      <section className="divide-y divide-line rounded-card border border-line">
        <Row label={t("settings.systemLanguage.label")}>
          {systemLocale ? (
            <span className="text-muted" title={systemLocale}>
              {localeName(systemLocale, uiLanguage)}
            </span>
          ) : (
            <span className="text-muted">{t("settings.systemLanguage.unknown")}</span>
          )}
        </Row>
        <UpdateSettings version={version} />
        <Row
          label={t("settings.logFolder.label")}
          description={t("settings.logFolder.description")}
        >
          <OpenLogFolder />
        </Row>
      </section>
    </>
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
    // Wraps the control under the label when the window is too narrow for both side by side.
    <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3 px-6 py-4">
      <div className="flex min-w-0 grow basis-48 flex-col gap-0.5 break-words">
        <span className="font-medium">{label}</span>
        {description && <span className="text-note text-muted">{description}</span>}
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
        className="hit-target rounded-md text-sm font-medium text-accent-strong underline-offset-4 hover:underline"
      >
        {t("settings.logFolder.open")}
      </button>
      {failed && (
        <span role="alert" className="text-note text-danger">
          {t("settings.logFolder.failed")}
        </span>
      )}
    </div>
  );
}
