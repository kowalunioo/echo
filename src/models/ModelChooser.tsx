import { type ReactNode, useEffect } from "react";
import { useTranslation } from "react-i18next";

import type { ModelEntry, ModelId, ModelsState } from "../bindings";
import { Button } from "../components/Button";
import { FailureMessage } from "../components/FailureMessage";
import { IconButton } from "../components/IconButton";
import { CheckIcon, CloseIcon, DownloadIcon, RefreshIcon } from "../components/icons";
import { SettingRow } from "../components/SettingRow";
import { useModels } from "../store/models";
import { StateLine } from "./ModelCard";
import { type CardState, cardState, megabytes } from "./view";

/**
 * The onboarding's "Choose a Model" step (settings-and-first-run.md rule 2.3): the three Models
 * as flat rows, each with its own Download, so the user can fetch more than one to try them (one
 * runs at a time, the rest queue: models.md rule 7). The `recommended` one is marked Recommended
 * (rule 30); progress, speed and Cancel show in the row that is
 * downloading, and a Model already on this computer offers "Use this Model".
 */
export function ModelChooser({ recommended }: { recommended: ModelId }) {
  const { t } = useTranslation();
  const models = useModels((s) => s.state);
  const load = useModels((s) => s.load);

  useEffect(() => {
    void load();
  }, [load]);

  if (!models) return null;
  return (
    <ul aria-label={t("onboarding.model.choose")}>
      {models.models.map((entry) => (
        <ChooserRow
          key={entry.id}
          entry={entry}
          models={models}
          recommended={entry.id === recommended}
        />
      ))}
    </ul>
  );
}

function ChooserRow({
  entry,
  models,
  recommended,
}: {
  entry: ModelEntry;
  models: ModelsState;
  recommended: boolean;
}) {
  const { t } = useTranslation();
  const state = cardState(entry, models);
  const loadFailure = models.loadFailure?.model === entry.id ? models.loadFailure : null;
  return (
    <SettingRow
      as="li"
      ariaLabel={entry.name}
      label={entry.name}
      // The size is on the Download button, so the line under the name says what the Model is.
      description={`${t(`models.languagesOf.${entry.id}`)} · ${t(`models.descriptions.${entry.id}`)}`}
      tag={
        <>
          {recommended && (
            <span className="shrink-0 text-xs font-medium text-accent-strong">
              {t("models.recommended")}
            </span>
          )}
          {state.kind === "active" && (
            <span className="flex shrink-0 items-center gap-1.5 rounded-full bg-accent-soft px-2 py-0.5 text-xs font-medium text-accent-soft-fg">
              <span aria-hidden="true" className="size-1.5 rounded-full bg-accent" />
              {t("models.active")}
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
        </>
      }
    >
      <ChooserAction entry={entry} state={state} loadingOther={models.activating !== null} />
    </SettingRow>
  );
}

/**
 * The one action a row's state allows, drawn like the App page's "Open log folder": borderless,
 * an icon in front, its text lined up with the row's edge. Every row's looks the same.
 */
function ChooserAction({
  entry,
  state,
  loadingOther,
}: {
  entry: ModelEntry;
  state: CardState;
  loadingOther: boolean;
}) {
  const { t } = useTranslation();
  const { download, cancel, activate } = useModels();
  const action = (icon: ReactNode, label: string, onClick: () => void, disabled = false) => (
    <Button
      variant="quiet"
      disabled={disabled}
      onClick={onClick}
      className="-mr-3.5 inline-flex items-center gap-1.5 tabular-nums"
    >
      {icon}
      {label}
    </Button>
  );

  switch (state.kind) {
    case "notDownloaded":
      return action(
        <DownloadIcon />,
        t("models.actions.download", { size: megabytes(entry.sizeBytes) }),
        () => void download(entry.id),
      );
    case "queued":
    case "downloading":
      // Named for the Model and what it stops: several rows can show this ✕ at once.
      return (
        <IconButton
          label={t(
            state.kind === "queued"
              ? "models.actions.removeFromQueue"
              : "models.actions.cancelDownload",
            { model: entry.name },
          )}
          onClick={() => void cancel(entry.id)}
        >
          <CloseIcon />
        </IconButton>
      );
    case "paused":
      return action(<DownloadIcon />, t("models.actions.resume"), () => void download(entry.id));
    case "failed":
      return action(<RefreshIcon />, t("models.actions.retry"), () => void download(entry.id));
    case "downloaded":
      return action(
        <CheckIcon />,
        t("models.actions.use"),
        () => void activate(entry.id),
        loadingOther,
      );
    case "verifying":
    case "loading":
    case "active":
      return null;
  }
}
