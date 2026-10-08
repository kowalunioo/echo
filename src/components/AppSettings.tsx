import { useState } from "react";
import { useTranslation } from "react-i18next";

import { type UiLanguage, commands } from "../bindings";
import { useSetting } from "../store/settings";
import { useShell } from "../store/shell";
import { AutostartSettings } from "./AutostartSettings";
import { Button } from "./Button";
import { FolderIcon, InfoIcon } from "./icons";
import { OverlaySettings } from "./OverlaySettings";
import { SectionHeading } from "./SectionHeading";
import { SettingRow } from "./SettingRow";
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
      <section aria-labelledby="app-general-heading">
        <SectionHeading id="app-general-heading">{t("pages.app.sections.general")}</SectionHeading>
        <SettingRow
          label={t("settings.uiLanguage.label")}
          description={t("settings.uiLanguage.description")}
        >
          <UiLanguagePicker />
        </SettingRow>
        <AutostartSettings />
      </section>
      <section aria-labelledby="app-overlay-heading">
        <SectionHeading id="app-overlay-heading">{t("pages.app.sections.overlay")}</SectionHeading>
        <OverlaySettings />
      </section>
      <section aria-labelledby="app-updates-heading">
        <SectionHeading id="app-updates-heading">{t("pages.app.sections.updates")}</SectionHeading>
        <UpdateSettings version={version} />
      </section>
      <section aria-labelledby="app-about-heading">
        <SectionHeading id="app-about-heading">{t("pages.app.sections.about")}</SectionHeading>
        <SettingRow label={t("settings.systemLanguage.label")}>
          {systemLocale ? (
            <span className="text-muted" title={systemLocale}>
              {localeName(systemLocale, uiLanguage)}
            </span>
          ) : (
            <span className="text-muted">{t("settings.systemLanguage.unknown")}</span>
          )}
        </SettingRow>
        <OpenLogFolder />
        {/* Explains, so it wraps rather than being cut (the one kind of text allowed to). */}
        <p className="flex items-start gap-2 pt-3 text-note text-muted">
          <InfoIcon className="mt-0.5 shrink-0" />
          <span>{t("settings.privacy")}</span>
        </p>
      </section>
    </>
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
    <SettingRow
      label={t("settings.logFolder.label")}
      description={t("settings.logFolder.description")}
      below={
        failed && (
          <span role="alert" className="text-note text-danger">
            {t("settings.logFolder.failed")}
          </span>
        )
      }
    >
      {/* Borderless: the text lines up with the row's edge, the hover fill reaches past it. */}
      <Button
        variant="quiet"
        onClick={() => void open()}
        className="-mr-3.5 inline-flex items-center gap-1.5"
      >
        <FolderIcon />
        {t("settings.logFolder.open")}
      </Button>
    </SettingRow>
  );
}
