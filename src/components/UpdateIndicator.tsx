import { type RefObject, useEffect, useLayoutEffect, useRef, useState } from "react";
import { type TFunction } from "i18next";
import { useTranslation } from "react-i18next";

import type { UpdateStatus } from "../bindings";
import { useShell } from "../store/shell";
import { useUpdater } from "../store/updater";

type Tone = "idle" | "ready" | "busy" | "news" | "error";

const BUSY = new Set<UpdateStatus["state"]>(["checking", "downloading", "installing"]);
const FAILED = new Set<UpdateStatus["state"]>([
  "checkFailed",
  "unverified",
  "downloadFailed",
  "installFailed",
]);

/** The updater's status line, as the App page words it; `null` when there is nothing to say. */
function statusText(t: TFunction, status: UpdateStatus): string | null {
  switch (status.state) {
    case "idle":
      return null;
    case "downloading":
      return status.percent === null
        ? t("settings.updates.status.downloadingUnknown")
        : t("settings.updates.status.downloading", { percent: status.percent });
    case "available":
    case "ready":
      return t(`settings.updates.status.${status.state}`, { version: status.version });
    default:
      return t(`settings.updates.status.${status.state}`);
  }
}

function toneOf(status: UpdateStatus): Tone {
  if (BUSY.has(status.state)) return "busy";
  if (FAILED.has(status.state)) return "error";
  if (status.state === "available" || status.state === "ready") return "news";
  if (status.state === "upToDate") return "ready";
  return "idle";
}

/**
 * The sidebar's last line (updater.md "UI"): "Echo 0.1.0 | Check for updates". The check runs
 * from here and the updater's status takes the link's place while there is one. A newer version
 * turns the link into "Install and restart" (rule 7); a failure opens the App page, where the
 * status line explains it.
 */
export function UpdateIndicator() {
  const { t } = useTranslation();
  const view = useUpdater((s) => s.view);
  const load = useUpdater((s) => s.load);
  const check = useUpdater((s) => s.check);
  const install = useUpdater((s) => s.install);
  const setPage = useShell((s) => s.setPage);

  useEffect(() => {
    void load();
  }, [load]);

  const { status, managed, currentVersion } = view ?? {
    status: { state: "idle" as const },
    managed: false,
    currentVersion: "",
  };
  const text = statusText(t, status);
  const tone = toneOf(status);
  const canCheck = !managed && (status.state === "idle" || status.state === "upToDate");
  const offersInstall = tone === "news";
  const label = offersInstall
    ? t("settings.updates.install")
    : (text ?? t("settings.updates.check"));
  const line = useRef<HTMLDivElement>(null);
  const stacked = useStacked(line, view !== null && !managed, label);
  if (!view) return null;
  // A newer version gets the one filled control of the sidebar: a small white pill (lavender is
  // kept for state marks, never for a call to action).
  const look = {
    idle: "text-muted hover:text-fg",
    ready: "text-muted hover:text-fg",
    busy: "text-muted",
    news: "rounded-full bg-fg px-2.5 py-0.5 font-medium text-bg hover:opacity-90 active:scale-[0.97] motion-reduce:active:scale-100",
    error: "text-danger",
  }[tone];

  return (
    <div
      ref={line}
      className={`flex items-center justify-center px-2.5 pt-1 text-xs text-muted ${
        stacked ? "flex-col" : "gap-2"
      }`}
    >
      <span
        className={`shrink-0 tabular-nums ${
          managed || stacked ? "" : "border-r border-control/60 pr-2 leading-3"
        }`}
        data-testid="status-version"
      >
        {t("app.name")} {currentVersion}
      </span>
      {!managed && (
        <button
          type="button"
          onClick={() => {
            if (offersInstall) void install();
            else if (canCheck) void check();
            else setPage("app");
          }}
          title={text ?? undefined}
          className={`min-w-0 truncate rounded-sm py-1 text-left transition-[opacity,color,transform] duration-150 ${look}`}
        >
          {label}
        </button>
      )}
    </div>
  );
}

/**
 * Whether the line has to stack the version over the button: "Zainstaluj i uruchom ponownie"
 * does not fit beside "Echo 0.1.0" in Polish. The text widths are measured, so the answer does
 * not depend on the layout it is measured in; a ResizeObserver re-measures when the text changes.
 */
function useStacked(line: RefObject<HTMLDivElement | null>, both: boolean, label: string) {
  const [stacked, setStacked] = useState(false);
  useLayoutEffect(() => {
    const element = line.current;
    if (!element || !both || typeof ResizeObserver === "undefined") return;
    const textWidth = (node: Element) => {
      const range = document.createRange();
      range.selectNodeContents(node);
      return range.getBoundingClientRect().width;
    };
    const measure = () => {
      const version = element.querySelector("[data-testid=status-version]");
      const button = element.querySelector("button");
      if (!version || !button) return;
      const inner = element.clientWidth - 20; // px-2.5
      // The button's natural width (text and any pill padding), the gap, the divider and its
      // padding.
      setStacked(textWidth(version) + 8 + 1 + 8 + button.scrollWidth > inner);
    };
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    const button = element.querySelector("button");
    if (button) observer.observe(button);
    return () => {
      observer.disconnect();
    };
    // `label` re-attaches the observer when the text changes, and after the first load, when the
    // line is first rendered.
  }, [line, both, label]);
  return stacked && both;
}
