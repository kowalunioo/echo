import { useTranslation } from "react-i18next";

import type { OverlayPosition } from "../bindings";
import { useSetting } from "../store/settings";
import { Row } from "./AppSettings";

const POSITIONS: readonly OverlayPosition[] = ["bottom", "top"];

/**
 * "Show recording indicator" and its position (overlay.md "Settings" and "UI"). Both apply at
 * once: the backend hides or moves a visible Overlay immediately (rule 16).
 */
export function OverlaySettings() {
  const { t } = useTranslation();
  const [show, setShow] = useSetting("showOverlay");
  const [position, setPosition] = useSetting("overlayPosition");

  return (
    <>
      <Row label={t("settings.overlay.label")} description={t("settings.overlay.description")}>
        <button
          type="button"
          role="switch"
          aria-checked={show}
          aria-label={t("settings.overlay.label")}
          onClick={() => void setShow(!show)}
          className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
            show ? "bg-accent-strong" : "border border-line bg-raised"
          }`}
        >
          <span
            aria-hidden
            className={`inline-block size-4 rounded-full shadow transition-transform ${
              show ? "translate-x-6 bg-accent-fg" : "translate-x-1 bg-muted"
            }`}
          />
        </button>
      </Row>
      <Row label={t("settings.overlay.position")}>
        <fieldset className="flex rounded-lg bg-raised p-0.5">
          <legend className="sr-only">{t("settings.overlay.position")}</legend>
          {POSITIONS.map((value) => (
            <label
              key={value}
              className={`cursor-pointer rounded-md px-3 py-1 text-sm transition-colors duration-150 has-focus-visible:outline-2 has-focus-visible:outline-focus ${
                position === value ? "bg-surface font-medium shadow-sm" : "text-muted hover:text-fg"
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
      </Row>
    </>
  );
}
