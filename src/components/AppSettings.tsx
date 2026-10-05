import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { UI_LANGUAGES } from "../i18n";
import { useShell } from "../store/shell";

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
      <Row label={t("settings.systemLanguage.label")}>
        <span className="text-muted">{systemLanguage}</span>
      </Row>
      <Row label={t("settings.version.label")}>
        <span className="text-muted tabular-nums" data-testid="app-version">
          {version}
        </span>
      </Row>
    </section>
  );
}

function Row({
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

function UiLanguagePicker() {
  const { t } = useTranslation();
  const uiLanguage = useShell((s) => s.uiLanguage);
  const chooseUiLanguage = useShell((s) => s.chooseUiLanguage);

  return (
    <fieldset className="flex rounded-lg bg-raised p-0.5">
      <legend className="sr-only">{t("settings.uiLanguage.label")}</legend>
      {UI_LANGUAGES.map((language) => (
        <label
          key={language}
          className={`cursor-pointer rounded-md px-3 py-1 text-sm transition-colors duration-150 has-focus-visible:outline-2 has-focus-visible:outline-focus ${
            uiLanguage === language
              ? "bg-surface font-medium shadow-sm"
              : "text-muted hover:text-fg"
          }`}
        >
          <input
            type="radio"
            name="ui-language"
            value={language}
            checked={uiLanguage === language}
            onChange={() => void chooseUiLanguage(language)}
            className="sr-only"
          />
          {t(`languages.${language}`)}
        </label>
      ))}
    </fieldset>
  );
}
