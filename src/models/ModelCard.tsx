import { type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";

import type { ModelEntry, ModelsState } from "../bindings";
import { useModels } from "../store/models";
import { Button } from "../components/Button";
import { ConfirmDialog } from "../components/Dialog";
import { FailureMessage } from "../components/FailureMessage";
import { type CardState, bytesOnDisk, cardState, megabytes, speed } from "./view";

/** One Model on the Models page: facts, state and the actions that state allows. */
export function ModelCard({ entry, models }: { entry: ModelEntry; models: ModelsState }) {
  const { t } = useTranslation();
  const state = cardState(entry, models);
  const [confirming, setConfirming] = useState(false);
  const remove = useModels((s) => s.remove);
  const loadFailure = models.loadFailure?.model === entry.id ? models.loadFailure : null;

  return (
    <li
      aria-label={entry.name}
      className={`flex flex-col gap-4 rounded-card border px-6 py-5 ${
        state.kind === "active" ? "border-accent bg-accent-soft/30" : "border-line bg-surface"
      }`}
    >
      <div className="flex items-start justify-between gap-6">
        <div className="flex min-w-0 flex-col gap-1">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="text-heading">{entry.name}</h3>
            {entry.recommended && <Badge tone="soft">{t("models.recommended")}</Badge>}
            {state.kind === "active" && <Badge tone="strong">{t("models.active")}</Badge>}
          </div>
          <p className="text-muted">{t(`models.descriptions.${entry.id}`)}</p>
          <p className="text-note text-muted">
            {t("models.size", { size: megabytes(entry.sizeBytes) })} ·{" "}
            {t(`models.languagesOf.${entry.id}`)}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Actions
            entry={entry}
            state={state}
            locked={models.dictationInProgress}
            loadingOther={models.activating !== null && models.activating !== entry.id}
            onDelete={() => {
              setConfirming(true);
            }}
          />
        </div>
      </div>

      <StateLine entry={entry} state={state} />

      {loadFailure && (
        <FailureMessage
          message={t("models.loadFailed", { model: entry.name })}
          detail={t("models.failureDetail", { detail: loadFailure.reason })}
          className="rounded-lg bg-danger-soft px-3 py-2 text-sm text-danger"
        />
      )}

      {confirming && (
        <ConfirmDialog
          title={t("models.confirmDelete.title", { model: entry.name })}
          body={t("models.confirmDelete.body", { size: megabytes(bytesOnDisk(entry)) })}
          confirm={t("models.confirmDelete.confirm")}
          cancel={t("models.confirmDelete.cancel")}
          onConfirm={() => {
            setConfirming(false);
            void remove(entry.id);
          }}
          onCancel={() => {
            setConfirming(false);
          }}
        />
      )}
    </li>
  );
}

function Actions({
  entry,
  state,
  locked,
  loadingOther,
  onDelete,
}: {
  entry: ModelEntry;
  state: CardState;
  locked: boolean;
  loadingOther: boolean;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  const { download, cancel, activate } = useModels();
  const deleteButton = (
    <Button variant="quiet" disabled={locked} onClick={onDelete}>
      {t("models.actions.delete")}
    </Button>
  );

  switch (state.kind) {
    case "notDownloaded":
      return (
        <Button variant="primary" onClick={() => void download(entry.id)}>
          {t("models.actions.download", { size: megabytes(entry.sizeBytes) })}
        </Button>
      );
    case "queued":
    case "downloading":
      return (
        <Button variant="secondary" onClick={() => void cancel(entry.id)}>
          {t("models.actions.cancel")}
        </Button>
      );
    case "verifying":
    case "loading":
      return null;
    case "paused":
      return (
        <>
          {deleteButton}
          <Button variant="secondary" onClick={() => void download(entry.id)}>
            {t("models.actions.resume")}
          </Button>
        </>
      );
    case "failed":
      return (
        <>
          {state.entry.downloaded > 0 && deleteButton}
          <Button variant="secondary" onClick={() => void download(entry.id)}>
            {t("models.actions.retry")}
          </Button>
        </>
      );
    case "downloaded":
      return (
        <>
          {deleteButton}
          <Button
            variant="primary"
            disabled={locked || loadingOther}
            onClick={() => void activate(entry.id)}
          >
            {t("models.actions.use")}
          </Button>
        </>
      );
    case "active":
      return deleteButton;
  }
}

/** Progress, status and failure text under the card's header. */
export function StateLine({ entry, state }: { entry: ModelEntry; state: CardState }) {
  const { t } = useTranslation();
  switch (state.kind) {
    case "downloading":
    case "paused":
      return (
        <div className="flex flex-col gap-1.5">
          <ProgressBar
            percent={state.percent}
            label={t("models.progress", { model: entry.name })}
            paused={state.kind === "paused"}
          />
          <p className="text-note text-muted tabular-nums" aria-live="polite">
            {state.kind === "downloading"
              ? t("models.state.downloading", {
                  percent: state.percent,
                  speed: speed(state.mbPerSecond),
                })
              : t("models.state.paused", { percent: state.percent })}
          </p>
        </div>
      );
    case "queued":
      return <StatusText>{t("models.state.queued")}</StatusText>;
    case "verifying":
      return <StatusText pulse>{t("models.state.verifying")}</StatusText>;
    case "loading":
      return <StatusText pulse>{t("models.state.loading")}</StatusText>;
    case "failed":
      return (
        <div role="alert" className="flex flex-col gap-0.5 rounded-lg bg-danger-soft px-3 py-2">
          <p className="text-sm text-danger">
            {t(`models.failure.${state.entry.failure.kind}`, {
              needed: megabytes(state.entry.failure.neededBytes ?? 0),
            })}
          </p>
          {state.entry.failure.kind !== "diskSpace" && state.entry.failure.kind !== "corrupted" && (
            <p className="cursor-text text-note text-muted select-text">
              {state.entry.failure.detail}
            </p>
          )}
        </div>
      );
    default:
      return null;
  }
}

function StatusText({ children, pulse = false }: { children: ReactNode; pulse?: boolean }) {
  return (
    <p aria-live="polite" className="flex items-center gap-2 text-note text-muted">
      <span
        aria-hidden="true"
        className={`size-2 rounded-full bg-accent ${pulse ? "animate-pulse" : ""}`}
      />
      {children}
    </p>
  );
}

export function ProgressBar({
  percent,
  label,
  paused = false,
}: {
  percent: number;
  label: string;
  paused?: boolean;
}) {
  return (
    <div
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={percent}
      className="h-1.5 overflow-hidden rounded-full bg-raised"
    >
      <div
        className={`h-full rounded-full transition-[width] duration-200 ease-out ${
          paused ? "bg-muted/60" : "bg-accent"
        }`}
        style={{ width: `${String(percent)}%` }}
      />
    </div>
  );
}

function Badge({ tone, children }: { tone: "soft" | "strong"; children: ReactNode }) {
  return (
    <span
      className={`rounded-full px-2 py-0.5 text-xs font-medium ${
        tone === "soft" ? "bg-accent-soft text-accent-soft-fg" : "bg-accent-strong text-accent-fg"
      }`}
    >
      {children}
    </span>
  );
}
