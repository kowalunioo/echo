import { type ReactNode, useEffect, useId, useRef, useState } from "react";
import { Trans, useTranslation } from "react-i18next";

import type { HistoryEntry } from "../bindings";
import { CopyIcon, MinusIcon, PlusIcon, ReinsertIcon, TrashIcon } from "../components/icons";
import { PageHeader } from "../components/PageHeader";
import { formatDate } from "../i18n";
import { useRecordShortcut } from "../onboarding/sources";
import { useHistory } from "../store/history";
import { useSetting } from "../store/settings";

/** How long Undo is offered after a delete (history.md rule 12). */
export const UNDO_MS = 5_000;
/** How long the "Copied" confirmation stays (rule 11). */
const COPIED_MS = 2_000;
/** Texts longer than this start collapsed. */
const COLLAPSE_CHARS = 480;

export const HISTORY_LIMIT_MIN = 0;
export const HISTORY_LIMIT_MAX = 100;

type Notice =
  | { kind: "copied" }
  | { kind: "deleted"; entry: HistoryEntry }
  | { kind: "copyFailed" }
  | { kind: "reinsertFailed" };

/** The History page (history.md "UI"): the list, its actions, the limit and Clear all. */
export function HistoryPage() {
  const { t } = useTranslation();
  const status = useHistory((s) => s.status);
  const entries = useHistory((s) => s.entries);
  const load = useHistory((s) => s.load);
  const remove = useHistory((s) => s.remove);
  const restore = useHistory((s) => s.restore);
  const reinsert = useHistory((s) => s.reinsert);
  const [limit] = useSetting("historyLimit");
  const [notice, setNotice] = useNotice();
  const [confirmingClear, setConfirmingClear] = useState(false);

  useEffect(() => {
    void load();
  }, [load]);

  const copy = async (entry: HistoryEntry) => {
    try {
      await navigator.clipboard.writeText(entry.text);
      setNotice({ kind: "copied" }, COPIED_MS);
    } catch (error) {
      console.error("copy failed", error);
      setNotice({ kind: "copyFailed" }, UNDO_MS);
    }
  };

  const del = async (entry: HistoryEntry) => {
    const removed = await remove(entry.id);
    if (removed) setNotice({ kind: "deleted", entry: removed }, UNDO_MS);
  };

  const undo = async (entry: HistoryEntry) => {
    setNotice(null);
    await restore(entry);
  };

  const reinsertEntry = async (entry: HistoryEntry) => {
    if (!(await reinsert(entry.id))) setNotice({ kind: "reinsertFailed" }, UNDO_MS);
  };

  return (
    <article className="mx-auto flex max-w-2xl flex-col gap-6 px-10 py-12">
      <PageHeader page="history">
        <LimitControl />
      </PageHeader>

      {status === "error" && (
        <p role="alert" className="text-muted">
          {t("history.loadFailed")}
        </p>
      )}

      {status === "ready" && entries.length === 0 && <EmptyState off={limit === 0} />}

      {entries.length > 0 && (
        <section className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-medium text-muted">{t("history.list")}</h2>
            <button
              type="button"
              onClick={() => {
                setConfirmingClear(true);
              }}
              className="rounded-md px-2 py-1 text-sm font-medium text-muted transition-colors duration-150 hover:bg-raised hover:text-fg"
            >
              {t("history.clearAll.button")}
            </button>
          </div>
          <ul aria-label={t("history.list")} className="flex flex-col gap-3">
            {entries.map((entry) => (
              <EntryItem
                key={entry.id}
                entry={entry}
                onCopy={() => void copy(entry)}
                onReinsert={() => void reinsertEntry(entry)}
                onDelete={() => void del(entry)}
              />
            ))}
          </ul>
        </section>
      )}

      {confirmingClear && (
        <ConfirmClear
          onCancel={() => {
            setConfirmingClear(false);
          }}
          onConfirm={() => {
            setConfirmingClear(false);
            setNotice(null);
            void useHistory.getState().clear();
          }}
        />
      )}

      <NoticeBar notice={notice} onUndo={(entry) => void undo(entry)} />
    </article>
  );
}

/** The current notice and a setter that hides it again after `ms`. */
function useNotice(): [Notice | null, (notice: Notice | null, ms?: number) => void] {
  const [notice, setNotice] = useState<Notice | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(
    () => () => {
      clearTimeout(timer.current);
    },
    [],
  );

  const show = (next: Notice | null, ms?: number) => {
    clearTimeout(timer.current);
    setNotice(next);
    if (next && ms) {
      timer.current = setTimeout(() => {
        setNotice(null);
      }, ms);
    }
  };
  return [notice, show];
}

/** The History limit field with steppers, in the page header (rules 6–9). */
function LimitControl() {
  const { t } = useTranslation();
  const [limit, setLimit] = useSetting("historyLimit");
  const [draft, setDraft] = useState<string | null>(null);
  const id = useId();

  const commit = () => {
    if (draft === null) return;
    const value = Number(draft);
    const valid =
      draft.trim() !== "" &&
      Number.isInteger(value) &&
      value >= HISTORY_LIMIT_MIN &&
      value <= HISTORY_LIMIT_MAX;
    if (valid && value !== limit) void setLimit(value);
    setDraft(null);
  };

  const step = (delta: number) => {
    setDraft(null);
    const next = Math.min(HISTORY_LIMIT_MAX, Math.max(HISTORY_LIMIT_MIN, limit + delta));
    if (next !== limit) void setLimit(next);
  };

  return (
    <div className="flex shrink-0 flex-col items-end gap-1.5">
      <label htmlFor={id} className="text-xs font-medium text-muted">
        {t("history.limit.label")}
      </label>
      <div className="flex items-center rounded-lg border border-line bg-surface">
        <StepButton
          label={t("history.limit.decrease")}
          disabled={limit <= HISTORY_LIMIT_MIN}
          onClick={() => {
            step(-1);
          }}
        >
          <MinusIcon />
        </StepButton>
        <input
          id={id}
          type="number"
          inputMode="numeric"
          min={HISTORY_LIMIT_MIN}
          max={HISTORY_LIMIT_MAX}
          step={1}
          value={draft ?? String(limit)}
          onChange={(e) => {
            setDraft(e.target.value);
          }}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
            if (e.key === "Escape") setDraft(null);
          }}
          className="w-12 [appearance:textfield] bg-transparent py-1 text-center tabular-nums outline-none [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
        />
        <StepButton
          label={t("history.limit.increase")}
          disabled={limit >= HISTORY_LIMIT_MAX}
          onClick={() => {
            step(1);
          }}
        >
          <PlusIcon />
        </StepButton>
      </div>
      <p className="text-xs text-muted">{t("history.limit.keeps", { count: limit })}</p>
    </div>
  );
}

function StepButton({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      className="flex size-8 items-center justify-center rounded-lg text-muted transition-colors duration-150 hover:bg-raised hover:text-fg disabled:pointer-events-none disabled:opacity-40"
    >
      {children}
    </button>
  );
}

function EmptyState({ off }: { off: boolean }) {
  const shortcut = useRecordShortcut();
  const { t } = useTranslation();
  return (
    <section className="flex flex-col items-center gap-2 rounded-card border border-dashed border-line px-6 py-12 text-center text-muted">
      {off ? (
        <p>{t("history.off")}</p>
      ) : (
        <p>
          <Trans
            i18nKey="history.empty"
            values={{ shortcut: shortcut.label }}
            components={{
              kbd: (
                <kbd className="rounded-md border border-line bg-raised px-1.5 py-0.5 font-sans text-sm font-medium text-fg" />
              ),
            }}
          />
        </p>
      )}
    </section>
  );
}

/** "5 października 2026, 14:03" / "5 October 2026, 14:03", in local time. */
function formatEntryTime(createdAt: number): string {
  return `${formatDate(createdAt, { dateStyle: "long" })}, ${formatDate(createdAt, { timeStyle: "short" })}`;
}

function EntryItem({
  entry,
  onCopy,
  onReinsert,
  onDelete,
}: {
  entry: HistoryEntry;
  onCopy: () => void;
  onReinsert: () => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  const collapsible = entry.text.length > COLLAPSE_CHARS;
  const [expanded, setExpanded] = useState(false);
  const textId = useId();

  return (
    <li className="group flex flex-col gap-2 rounded-card border border-line bg-surface px-5 py-4">
      <div className="flex items-center justify-between gap-4">
        <time
          dateTime={new Date(entry.createdAt).toISOString()}
          className="text-xs text-muted tabular-nums"
        >
          {formatEntryTime(entry.createdAt)}
        </time>
        <div className="flex items-center gap-0.5">
          <ActionButton label={t("history.copy")} onClick={onCopy}>
            <CopyIcon />
          </ActionButton>
          <ActionButton label={t("history.reinsert")} onClick={onReinsert}>
            <ReinsertIcon />
          </ActionButton>
          <ActionButton label={t("history.delete")} onClick={onDelete}>
            <TrashIcon />
          </ActionButton>
        </div>
      </div>
      <p
        id={textId}
        data-entry-text
        className={`cursor-text break-words whitespace-pre-wrap select-text ${
          collapsible && !expanded ? "line-clamp-5" : ""
        }`}
      >
        {entry.text}
      </p>
      {collapsible && (
        <button
          type="button"
          aria-expanded={expanded}
          aria-controls={textId}
          onClick={() => {
            setExpanded(!expanded);
          }}
          className="self-start rounded-md text-sm font-medium text-accent-strong underline-offset-4 hover:underline"
        >
          {t(expanded ? "history.showLess" : "history.showMore")}
        </button>
      )}
    </li>
  );
}

function ActionButton({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className="flex size-8 items-center justify-center rounded-lg text-muted transition-colors duration-150 hover:bg-raised hover:text-fg"
    >
      {children}
    </button>
  );
}

/** The Clear all confirmation (rule 14). Cancel has focus, so Enter never deletes by accident. */
function ConfirmClear({ onCancel, onConfirm }: { onCancel: () => void; onConfirm: () => void }) {
  const { t } = useTranslation();
  const titleId = useId();
  const bodyId = useId();
  const cancel = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    cancel.current?.focus();
  }, []);

  return (
    <div
      className="fixed inset-0 z-20 flex items-center justify-center bg-black/30 p-6"
      onKeyDown={(e) => {
        if (e.key === "Escape") onCancel();
      }}
    >
      <div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={bodyId}
        className="flex w-full max-w-sm flex-col gap-3 rounded-card border border-line bg-surface p-6 shadow-xl"
      >
        <h2 id={titleId} className="font-display text-lg font-semibold">
          {t("history.clearAll.title")}
        </h2>
        <p id={bodyId} className="text-muted">
          {t("history.clearAll.body")}
        </p>
        <div className="mt-2 flex justify-end gap-2">
          <button
            ref={cancel}
            type="button"
            onClick={onCancel}
            className="rounded-lg px-3 py-1.5 font-medium transition-colors duration-150 hover:bg-raised"
          >
            {t("history.clearAll.cancel")}
          </button>
          <button
            type="button"
            onClick={onConfirm}
            className="rounded-lg bg-accent-strong px-3 py-1.5 font-medium text-accent-fg transition-opacity duration-150 hover:opacity-90"
          >
            {t("history.clearAll.confirm")}
          </button>
        </div>
      </div>
    </div>
  );
}

function NoticeBar({
  notice,
  onUndo,
}: {
  notice: Notice | null;
  onUndo: (entry: HistoryEntry) => void;
}) {
  const { t } = useTranslation();
  if (!notice) return null;
  const failure = notice.kind === "copyFailed" || notice.kind === "reinsertFailed";
  const text = {
    copied: t("history.copied"),
    deleted: t("history.deleted"),
    copyFailed: t("history.copyFailed"),
    reinsertFailed: t("history.reinsertFailed"),
  }[notice.kind];

  return (
    <div className="pointer-events-none fixed inset-x-0 bottom-6 z-10 flex justify-center px-6">
      <div
        role={failure ? "alert" : "status"}
        className="pointer-events-auto flex items-center gap-4 rounded-xl bg-fg px-4 py-2.5 text-sm text-bg shadow-lg"
      >
        <span>{text}</span>
        {notice.kind === "deleted" && (
          <button
            type="button"
            onClick={() => {
              onUndo(notice.entry);
            }}
            className="rounded-md font-medium text-accent-soft underline-offset-4 hover:underline"
          >
            {t("history.undo")}
          </button>
        )}
      </div>
    </div>
  );
}
