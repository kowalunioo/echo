import { useTranslation } from "react-i18next";

import { UI_LANGUAGES } from "../i18n";
import { useSetting } from "../store/settings";

/** The Polski / English switch, on Welcome and in the App section. Saves at once (rule 11). */
export function UiLanguagePicker() {
  const { t } = useTranslation();
  const [uiLanguage, setUiLanguage] = useSetting("uiLanguage");

  return (
    <fieldset className="flex rounded-lg border border-line p-0.5">
      <legend className="sr-only">{t("settings.uiLanguage.label")}</legend>
      {UI_LANGUAGES.map((language) => (
        <label
          key={language}
          className={`cursor-pointer rounded-md px-3 py-1 text-sm transition-colors duration-150 has-focus-visible:outline-2 has-focus-visible:outline-focus ${
            uiLanguage === language ? "bg-raised font-medium text-fg" : "text-muted hover:text-fg"
          }`}
        >
          <input
            type="radio"
            name="ui-language"
            value={language}
            checked={uiLanguage === language}
            onChange={() => void setUiLanguage(language)}
            className="sr-only"
          />
          {t(`languages.${language}`)}
        </label>
      ))}
    </fieldset>
  );
}
