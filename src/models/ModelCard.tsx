import { type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";

import type { ModelEntry, ModelsState } from "../bindings";
import { useModels } from "../store/models";
import { Button } from "../components/Button";
import { ConfirmDialog } from "../components/Dialog";
import { FailureMessage } from "../components/FailureMessage";
import { IconButton } from "../components/IconButton";
import { CloseIcon, TrashIcon } from "../components/icons";
import { SettingRow } from "../components/SettingRow";
import { type CardState, bytesOnDisk, cardState, megabytes, speed } from "./view";

/**
 * One Model on the Models page as a flat row: name, a one-line description with size and
 * languages, the actions its state allows on the right, and progress or failure under it.
 */
export function ModelCard({ entry, models }: { entry: ModelEntry; models: ModelsState }) {
  const { t } = useTranslation();
  const state = cardState(entry, models);
  const [confirming, setConfirming] = useState(false);
  const remove = useModels((s) => s.remove);
  const loadFailure = models.loadFailure?.model === entry.id ? models.loadFailure : null;

  return (
    <SettingRow
      as="li"
      ariaLabel={entry.name}
      label={entry.name}
      prominent={state.kind === "active"}
      description={`${t(`models.descriptions.${entry.id}`)} · ${t("models.size", {
        size: megabytes(entry.sizeBytes),
      })} · ${t(`models.languagesOf.${entry.id}`)}`}
      tag={
        <>
          {state.kind === "active" && (
            <span className="flex shrink-0 items-center gap-1.5 rounded-full bg-accent-soft px-2 py-0.5 text-xs font-medium text-accent-soft-fg">
              <span aria-hidden="true" className="size-1.5 rounded-full bg-accent" />
              {t("models.active")}
            </span>
          )}
          {entry.recommended && (
            <span className="shrink-0 rounded-full bg-raised px-2 py-0.5 text-xs font-medium text-muted">
              {t("models.recommended")}
            </span>
          )}
        </>
      }
      below={
        <>
          <StateLine entry={entry} state={state} />
          {loadFailure && (
            <FailureMessage
              message={t("models.loadFailed", { model: entry.name })}
              detail={t("models.failureDetail", { detail: loadFailure.reason })}
              className="text-note text-danger"
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
        </>
      }
    >
      <Actions
        entry={entry}
        state={state}
        locked={models.dictationInProgress}
        loadingOther={models.activating !== null && models.activating !== entry.id}
        onDelete={() => {
          setConfirming(true);
        }}
      />
    </SettingRow>
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
    <IconButton label={t("models.actions.delete")} disabled={locked} onClick={onDelete}>
      <TrashIcon />
    </IconButton>
  );

  switch (state.kind) {
    case "notDownloaded":
      return (
        <Button variant="contrast" onClick={() => void download(entry.id)}>
          {t("models.actions.download", { size: megabytes(entry.sizeBytes) })}
        </Button>
      );
    case "queued":
    case "downloading":
      return (
        <IconButton label={t("models.actions.cancel")} onClick={() => void cancel(entry.id)}>
          <CloseIcon />
        </IconButton>
      );
    case "verifying":
    case "loading":
      return null;
    case "paused":
      return (
        <>
          <Button variant="contrast" onClick={() => void download(entry.id)}>
            {t("models.actions.resume")}
          </Button>
          {deleteButton}
        </>
      );
    case "failed":
      return (
        <>
          <Button variant="contrast" onClick={() => void download(entry.id)}>
            {t("models.actions.retry")}
          </Button>
          {state.entry.downloaded > 0 && deleteButton}
        </>
      );
    case "downloaded":
      return (
        <>
          <Button
            variant="contrast"
            disabled={locked || loadingOther}
            onClick={() => void activate(entry.id)}
          >
            {t("models.actions.use")}
          </Button>
          {deleteButton}
        </>
      );
    case "active":
      return (
        <>
          <ActiveState />
          {deleteButton}
        </>
      );
  }
}

/**
 * The active Model's memory state when it needs attention, in the words of the sidebar's Model
 * indicator: a pulsing dot while loading, grey when freed, red on error. Ready says nothing.
 */
function ActiveState() {
  const { t } = useTranslation();
  const activeState = useModels((s) => s.state?.activeState ?? "none");
  if (activeState === "ready" || activeState === "none") return null;
  const dot = {
    loading: "bg-accent animate-pulse",
    unloaded: "bg-muted/50",
    error: "bg-danger",
  }[activeState];
  const text = t(`models.indicator.${activeState}`);
  return (
    <span
      title={text}
      className={`flex max-w-52 items-center gap-2 text-note ${
        activeState === "error" ? "text-danger" : "text-muted"
      }`}
    >
      <span aria-hidden="true" className={`size-2 shrink-0 rounded-full ${dot}`} />
      <span className="truncate">{text}</span>
    </span>
  );
}

/** Progress, status and failure text under the card's header. */
export function StateLine({ entry, state }: { entry: ModelEntry; state: CardState }) {
  const { t } = useTranslation();
  switch (state.kind) {
    case "downloading":
    case "paused":
      return (
        <div className="flex items-center gap-3">
          <div className="min-w-0 flex-1">
            <ProgressBar
              percent={state.percent}
              label={t("models.progress", { model: entry.name })}
              paused={state.kind === "paused"}
            />
          </div>
          <p className="shrink-0 text-note text-muted tabular-nums" aria-live="polite">
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
        <div role="alert" className="flex flex-col gap-0.5">
          <p className="text-note text-danger">
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
