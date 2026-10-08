import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import type { ModelId, ModelsState } from "../bindings";
import { FailureMessage } from "../components/FailureMessage";
import { useModels } from "../store/models";
import { Button } from "../components/Button";
import { StateLine } from "./ModelCard";
import { cardState, megabytes } from "./view";

/**
 * The onboarding's "Choose a Model" step (settings-and-first-run.md rule 2.3): the three Models,
 * the `recommended` one pre-selected and badged (models.md rule 30), and Download with progress,
 * speed and Cancel — or "Use this Model" for one already on this computer. The step completes
 * once a Model is active.
 */
export function ModelChooser({ recommended }: { recommended: ModelId }) {
  const { t } = useTranslation();
  const models = useModels((s) => s.state);
  const load = useModels((s) => s.load);
  const [selected, setSelected] = useState<ModelId>(recommended);

  useEffect(() => {
    void load();
  }, [load]);

  if (!models) return null;
  return (
    <div className="flex flex-col gap-5">
      <fieldset className="flex flex-col gap-2">
        <legend className="sr-only">{t("onboarding.model.choose")}</legend>
        {models.models.map((entry) => {
          const state = cardState(entry, models);
          const checked = entry.id === selected;
          const status =
            state.kind === "downloaded" || state.kind === "active"
              ? t("models.state.downloaded")
              : null;
          return (
            <label
              key={entry.id}
              className={`flex cursor-pointer items-start gap-3 rounded-xl border px-4 py-3 transition-colors duration-150 has-focus-visible:outline-2 has-focus-visible:outline-focus ${
                checked ? "border-accent bg-accent-soft/50" : "border-line hover:bg-raised/40"
              }`}
            >
              <input
                type="radio"
                name="onboarding-model"
                value={entry.id}
                checked={checked}
                onChange={() => {
                  setSelected(entry.id);
                }}
                className="mt-1 accent-accent-strong"
              />
              <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                <span className="flex flex-wrap items-center gap-2">
                  <span className="font-medium">{entry.name}</span>
                  {entry.id === recommended && (
                    <span className="rounded-full bg-accent-soft px-2 py-0.5 text-xs font-medium text-accent-soft-fg">
                      {t("models.recommended")}
                    </span>
                  )}
                </span>
                <span className="text-sm text-muted">{t(`models.descriptions.${entry.id}`)}</span>
                <span className="text-note text-muted">
                  {t("models.size", { size: megabytes(entry.sizeBytes) })} ·{" "}
                  {t(`models.languagesOf.${entry.id}`)}
                  {status && ` · ${status}`}
                </span>
              </span>
            </label>
          );
        })}
      </fieldset>
      <SelectedAction id={selected} models={models} />
    </div>
  );
}

function SelectedAction({ id, models }: { id: ModelId; models: ModelsState }) {
  const { t } = useTranslation();
  const { download, cancel, activate } = useModels();
  const entry = models.models.find((m) => m.id === id);
  if (!entry) return null;
  const state = cardState(entry, models);
  const loadFailure = models.loadFailure?.model === id ? models.loadFailure : null;

  let action = null;
  switch (state.kind) {
    case "notDownloaded":
      action = (
        <Button size="default" variant="primary" onClick={() => void download(id)}>
          {t("models.actions.download", { size: megabytes(entry.sizeBytes) })}
        </Button>
      );
      break;
    case "queued":
    case "downloading":
      action = (
        <Button size="default" variant="secondary" onClick={() => void cancel(id)}>
          {t("models.actions.cancel")}
        </Button>
      );
      break;
    case "paused":
      action = (
        <Button size="default" variant="primary" onClick={() => void download(id)}>
          {t("models.actions.resume")}
        </Button>
      );
      break;
    case "failed":
      action = (
        <Button size="default" variant="primary" onClick={() => void download(id)}>
          {t("models.actions.retry")}
        </Button>
      );
      break;
    case "downloaded":
    case "active":
      action = (
        <Button
          size="default"
          variant="primary"
          disabled={models.activating !== null}
          onClick={() => void activate(id)}
        >
          {t("models.actions.use")}
        </Button>
      );
      break;
    case "verifying":
    case "loading":
      break;
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="min-h-6">
        <StateLine entry={entry} state={state} />
        {loadFailure && (
          <FailureMessage
            message={t("models.loadFailed", { model: entry.name })}
            detail={t("models.failureDetail", { detail: loadFailure.reason })}
            className="rounded-lg bg-danger-soft px-3 py-2 text-sm text-danger"
          />
        )}
      </div>
      {action && <div className="flex justify-end">{action}</div>}
    </div>
  );
}
