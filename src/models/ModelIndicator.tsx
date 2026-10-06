import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { useModels } from "../store/models";
import { useShell } from "../store/shell";
import { downloadingEntry, percentOf } from "./view";

type Tone = "ready" | "busy" | "idle" | "error";

/**
 * The compact Model indicator in the sidebar: the active Model and its state, or the call to
 * download one (models.md "UI", settings-and-first-run.md status area). Opens the Models page.
 */
export function ModelIndicator() {
  const { t } = useTranslation();
  const models = useModels((s) => s.state);
  const load = useModels((s) => s.load);
  const setPage = useShell((s) => s.setPage);

  useEffect(() => {
    void load();
  }, [load]);

  if (!models) return null;
  const active = models.models.find((m) => m.id === models.active);
  const loadingTarget = models.models.find((m) => m.id === models.activating);
  const downloading = downloadingEntry(models);

  let name: string;
  let detail: string;
  let tone: Tone;
  if (loadingTarget) {
    name = loadingTarget.name;
    detail = t("models.indicator.loading");
    tone = "busy";
  } else if (active) {
    name = active.name;
    const byState = {
      ready: ["ready", "ready"],
      loading: ["loading", "busy"],
      unloaded: ["unloaded", "idle"],
      error: ["error", "error"],
      none: ["ready", "ready"],
    } as const;
    const [key, stateTone] = byState[models.activeState];
    detail = t(`models.indicator.${key}`);
    tone = stateTone;
  } else if (downloading) {
    name = downloading.name;
    const progress =
      downloading.download.state === "downloading"
        ? percentOf(downloading.download.downloaded, downloading.download.total)
        : 100;
    detail = t("models.indicator.downloading", { percent: progress });
    tone = "busy";
  } else {
    name = t("models.indicator.none");
    detail = "";
    tone = "idle";
  }

  const dot = {
    ready: "bg-accent",
    busy: "bg-accent animate-pulse",
    idle: "bg-muted/50",
    error: "bg-danger",
  }[tone];

  return (
    <button
      type="button"
      onClick={() => {
        setPage("model");
      }}
      aria-label={`${t("models.indicator.open")}: ${name}${detail ? `, ${detail}` : ""}`}
      className="flex w-full items-start gap-2.5 rounded-lg border border-line bg-surface px-3 py-2 text-left transition-colors duration-150 hover:bg-raised/60"
    >
      <span className={`mt-1.5 size-2 shrink-0 rounded-full ${dot}`} aria-hidden="true" />
      <span className="flex min-w-0 flex-col">
        <span className="truncate text-xs font-medium">{name}</span>
        {detail && (
          <span className={`text-xs ${tone === "error" ? "text-danger" : "text-muted"}`}>
            {detail}
          </span>
        )}
      </span>
    </button>
  );
}

/**
 * The prominent "Download a Model to start" panel shown after onboarding when no Model is active
 * (settings-and-first-run.md rule 5).
 */
export function NoModelPanel({ onModelsPage }: { onModelsPage: boolean }) {
  const { t } = useTranslation();
  const models = useModels((s) => s.state);
  const setPage = useShell((s) => s.setPage);

  if (!models || models.active !== null || models.activating !== null) return null;
  return (
    <section className="flex items-center justify-between gap-6 rounded-card border border-accent/40 bg-accent-soft px-6 py-5">
      <div className="flex flex-col gap-1">
        <h2 className="font-display text-base font-semibold text-accent-soft-fg">
          {t("models.start.title")}
        </h2>
        <p className="text-sm text-accent-soft-fg">{t("models.start.body")}</p>
      </div>
      {!onModelsPage && (
        <button
          type="button"
          onClick={() => {
            setPage("model");
          }}
          className="shrink-0 rounded-lg bg-accent-strong px-4 py-2 text-sm font-medium whitespace-nowrap text-accent-fg hover:opacity-90"
        >
          {t("models.start.action")}
        </button>
      )}
    </section>
  );
}
