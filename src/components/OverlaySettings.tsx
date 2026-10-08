import { useTranslation } from "react-i18next";

import type { OverlayPosition } from "../bindings";
import { useSetting } from "../store/settings";
import { SettingRow } from "./SettingRow";
import { Switch } from "./Switch";

const POSITIONS: readonly OverlayPosition[] = ["bottom", "top"];

/**
 * "Show the Overlay" and its position (overlay.md "Settings" and "UI"). Both apply at
 * once: the backend hides or moves a visible Overlay immediately (rule 16). While the Overlay is
 * off the position stays visible but cannot be changed: it would have no effect.
 */
export function OverlaySettings() {
  const { t } = useTranslation();
  const [show, setShow] = useSetting("showOverlay");
  const [position, setPosition] = useSetting("overlayPosition");

  return (
    <>
      <SettingRow
        label={t("settings.overlay.label")}
        description={t("settings.overlay.description")}
      >
        <Switch
          checked={show}
          label={t("settings.overlay.label")}
          onChange={(next) => void setShow(next)}
        />
      </SettingRow>
      <SettingRow label={t("settings.overlay.position")}>
        <fieldset
          disabled={!show}
          aria-disabled={!show}
          className={`flex rounded-lg border border-line p-0.5 ${show ? "" : "cursor-not-allowed opacity-50"}`}
        >
          <legend className="sr-only">{t("settings.overlay.position")}</legend>
          {POSITIONS.map((value) => (
            <label
              key={value}
              className={`rounded-md px-3 py-1 text-sm transition-colors duration-150 has-focus-visible:outline-2 has-focus-visible:outline-focus ${
                show ? "cursor-pointer" : "pointer-events-none"
              } ${
                position === value
                  ? "bg-selected font-medium text-fg"
                  : `text-muted ${show ? "hover:text-fg" : ""}`
              }`}
            >
              <input
                type="radio"
                name="overlay-position"
                value={value}
                checked={position === value}
                onChange={() => void setPosition(value)}
                className="sr-only"
              />
              {t(`settings.overlay.${value}`)}
            </label>
          ))}
        </fieldset>
      </SettingRow>
    </>
  );
}
