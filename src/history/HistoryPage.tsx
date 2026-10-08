import { type ReactNode, type RefObject, useEffect, useId, useRef, useState } from "react";
import { Trans, useTranslation } from "react-i18next";

import type { HistoryEntry } from "../bindings";
import {
  CopyIcon,
  EchoMark,
  MinusIcon,
  PlusIcon,
  ReinsertIcon,
  TrashIcon,
} from "../components/icons";
import { Keycap } from "../components/Keycap";
import { PageHeader } from "../components/PageHeader";
import { ConfirmDialog } from "../components/Dialog";
import { formatDate } from "../i18n";
import { useRecordShortcut } from "../onboarding/sources";
import { useHistory } from "../store/history";
import { useSetting } from "../store/settings";

/** How long Undo is offered after a delete (history.md rule 12). */
export const UNDO_MS = 5_000;
/** How long the "Copied" confirmation stays (rule 11). */
const COPIED_MS = 2_000;
/** The notice fades out for this long before it unmounts. */
const NOTICE_EXIT_MS = 150;
/** Texts longer than this start collapsed. */
const COLLAPSE_CHARS = 480;

export const HISTORY_LIMIT_MIN = 0;
export const HISTORY_LIMIT_MAX = 100;

type Notice =
  | { kind: "copied" }
  | { kind: "deleted"; entry: HistoryEntry }
  /** Lowering the limit removed `removed` (newest first); Undo puts `limit` and them back. */
  | { kind: "trimmed"; removed: HistoryEntry[]; limit: number }
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
  const [limit, setLimit] = useSetting("historyLimit");
  const [notice, setNotice] = useNotice();
  const [confirmingClear, setConfirmingClear] = useState(false);
  const list = useRef<HTMLUListElement>(null);
  const undoButton = useRef<HTMLButtonElement>(null);
  /** After a delete: the deleted entry and its position, until the list no longer shows it. */
  const focusAfterDelete = useRef<{ id: number; index: number } | null>(null);

  // Focus follows a delete to the next entry (or the new last one), or to Undo when the list is
  // gone, so keyboard users keep their place. Checked after every render, because the list
  // update and the command's answer can arrive in either order.
  useEffect(() => {
    const pending = focusAfterDelete.current;
    if (!pending || list.current?.querySelector(`[data-entry-id="${String(pending.id)}"]`)) return;
    focusAfterDelete.current = null;
    const buttons = list.current?.querySelectorAll<HTMLButtonElement>("[data-delete]") ?? [];
    const target = buttons[Math.min(pending.index, buttons.length - 1)] ?? undoButton.current;
    target?.focus();
  });

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
    const index = entries.findIndex((e) => e.id === entry.id);
    const removed = await remove(entry.id);
    if (removed) {
      setNotice({ kind: "deleted", entry: removed }, UNDO_MS);
      focusAfterDelete.current = { id: entry.id, index };
    }
  };

  /**
   * Rule 8: a lower limit removes the oldest surplus at once, with Undo for 5 s. Lowering it
   * again within those 5 s adds to the same Undo.
   */
  const changeLimit = async (next: number) => {
    const removed = entries.slice(next);
    const earlier = notice?.kind === "trimmed" ? notice : null;
    await setLimit(next);
    if (removed.length === 0) return;
    setNotice(
      {
        kind: "trimmed",
        removed: [...(earlier?.removed ?? []), ...removed],
        limit: earlier?.limit ?? limit,
      },
      UNDO_MS,
    );
  };

  const undo = async (current: Notice) => {
    setNotice(null);
    if (current.kind === "deleted") await restore(current.entry);
    if (current.kind === "trimmed") {
      // The limit first, so the restored entries fit.
      await setLimit(current.limit);
      for (const entry of current.removed) await restore(entry);
    }
  };

  const reinsertEntry = async (entry: HistoryEntry) => {
    if (!(await reinsert(entry.id))) setNotice({ kind: "reinsertFailed" }, UNDO_MS);
  };

  return (
    <article className="mx-auto flex max-w-2xl flex-col gap-6 px-10 py-12">
      <PageHeader page="history">
        <LimitControl
          onChange={(next) => {
            void changeLimit(next);
          }}
        />
      </PageHeader>

      {status === "error" && (
        <p role="alert" className="text-danger">
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
              className="rounded-md px-2 py-1 text-sm font-medium text-muted transition-[background-color,color,transform] duration-150 ease-out-strong hover:bg-raised hover:text-fg active:scale-[0.97] motion-reduce:active:scale-100"
            >
              {t("history.clearAll.button")}
            </button>
          </div>
          <ul
            ref={list}
            tabIndex={-1}
            aria-label={t("history.list")}
            className="flex flex-col gap-3 outline-none"
          >
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
        <ConfirmDialog
          title={t("history.clearAll.title")}
          body={t("history.clearAll.body")}
          confirm={t("history.clearAll.confirm")}
          cancel={t("history.clearAll.cancel")}
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

      <NoticeBar notice={notice} undoRef={undoButton} onUndo={(current) => void undo(current)} />
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
function LimitControl({ onChange }: { onChange: (limit: number) => void }) {
  const { t } = useTranslation();
  const [limit] = useSetting("historyLimit");
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
    if (valid && value !== limit) onChange(value);
    setDraft(null);
  };

  const step = (delta: number) => {
    setDraft(null);
    const next = Math.min(HISTORY_LIMIT_MAX, Math.max(HISTORY_LIMIT_MIN, limit + delta));
    if (next !== limit) onChange(next);
  };

  return (
    <div className="flex shrink-0 flex-col items-end gap-1.5">
      <label htmlFor={id} className="text-note font-medium text-muted">
        {t("history.limit.label")}
      </label>
      <div className="flex items-center rounded-lg border border-control bg-surface has-[input:focus-visible]:outline-2 has-[input:focus-visible]:outline-offset-2 has-[input:focus-visible]:outline-focus">
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
      <p className="text-note text-muted">{t("history.limit.keeps", { count: limit })}</p>
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
    <section className="flex flex-col items-center gap-3 rounded-card border border-dashed border-line px-6 py-12 text-center text-muted">
      <EchoMark className="size-9 text-muted/40" />
      {off ? (
        <p>{t("history.off")}</p>
      ) : (
        <p>
          <Trans
            i18nKey="history.empty"
            values={{ shortcut: shortcut.label }}
            components={{
              kbd: <Keycap />,
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
    <li
      data-entry-id={entry.id}
      className="group flex flex-col gap-2 rounded-card border border-line bg-surface px-5 py-4"
    >
      <div className="flex items-center justify-between gap-4">
        <time
          dateTime={new Date(entry.createdAt).toISOString()}
          className="text-note text-muted tabular-nums"
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
          <ActionButton label={t("history.delete")} onClick={onDelete} isDelete>
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
          className="hit-target self-start rounded-md text-sm font-medium text-accent-strong underline-offset-4 transition-transform duration-150 ease-out-strong hover:underline active:scale-[0.97] motion-reduce:active:scale-100"
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
  isDelete = false,
  children,
}: {
  label: string;
  onClick: () => void;
  /** Marks the Delete button, which takes focus after the entry before it is deleted. */
  isDelete?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      data-delete={isDelete || undefined}
      aria-label={label}
      title={label}
      onClick={onClick}
      className="flex size-8 items-center justify-center rounded-lg text-muted transition-[background-color,color,transform] duration-150 ease-out-strong hover:bg-raised hover:text-fg active:scale-95 motion-reduce:active:scale-100"
    >
      {children}
    </button>
  );
}

function NoticeBar({
  notice,
  undoRef,
  onUndo,
}: {
  notice: Notice | null;
  undoRef: RefObject<HTMLButtonElement | null>;
  onUndo: (notice: Notice) => void;
}) {
  const { t } = useTranslation();
  // Keeps the last notice mounted for the exit fade; a new notice cancels it at once.
  const [held, setHeld] = useState<Notice | null>(notice);
  if (notice && held !== notice) setHeld(notice);
  useEffect(() => {
    if (notice) return;
    const timer = setTimeout(() => {
      setHeld(null);
    }, NOTICE_EXIT_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [notice]);
  const shown = notice ?? held;
  if (!shown) return null;
  const leaving = notice === null;
  const failure = shown.kind === "copyFailed" || shown.kind === "reinsertFailed";
  const text = {
    copied: t("history.copied"),
    deleted: t("history.deleted"),
    trimmed: t("history.trimmed", {
      count: shown.kind === "trimmed" ? shown.removed.length : 0,
    }),
    copyFailed: t("history.copyFailed"),
    reinsertFailed: t("history.reinsertFailed"),
  }[shown.kind];

  return (
    <div
      aria-hidden={leaving || undefined}
      className="pointer-events-none fixed inset-x-0 bottom-6 z-10 flex justify-center px-6"
    >
      <div
        role={failure ? "alert" : "status"}
        className={`flex items-center gap-4 rounded-xl bg-fg px-4 py-2.5 text-sm text-bg shadow-lg transition-[opacity,transform] ease-out-strong starting:translate-y-full starting:opacity-0 motion-reduce:starting:translate-y-0 ${
          leaving
            ? "pointer-events-none translate-y-2 opacity-0 duration-150 motion-reduce:translate-y-0"
            : "pointer-events-auto duration-200"
        }`}
      >
        <span>{text}</span>
        {(shown.kind === "deleted" || shown.kind === "trimmed") && (
          <button
            ref={undoRef}
            type="button"
            onClick={() => {
              onUndo(shown);
            }}
            className="hit-target rounded-md font-medium text-accent-soft underline-offset-4 transition-transform duration-150 ease-out-strong hover:underline active:scale-[0.97] motion-reduce:active:scale-100"
          >
            {t("history.undo")}
          </button>
        )}
      </div>
    </div>
  );
}
