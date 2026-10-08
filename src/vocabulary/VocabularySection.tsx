import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "../components/Button";
import { EchoMark } from "../components/icons";
import { useModels } from "../store/models";
import { useSetting } from "../store/settings";
import {
  HINT_CHAR_BUDGET,
  MAX_ENTRY_CHARS,
  WARNING_PERCENT,
  acceptsPrompt,
  budgetPercent,
  checkEntry,
  hintChars,
} from "./vocabulary";

/** How long Undo is offered after a removal, as for History (history.md rule 12). */
export const UNDO_MS = 5_000;

interface Removal {
  entry: string;
  index: number;
}

/** The Vocabulary page: add field, entries as removable chips, hint budget (vocabulary.md "UI"). */
export function VocabularySection() {
  const { t } = useTranslation();
  const [entries, setEntries] = useSetting("vocabulary");
  const models = useModels((s) => s.state);
  const [input, setInput] = useState("");
  const [duplicate, setDuplicate] = useState<string | null>(null);
  const [removal, setRemoval] = useState<Removal | null>(null);
  const undoTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const list = useRef<HTMLUListElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  // The chip whose remove control takes focus once the list has re-rendered after a removal.
  const focusAfter = useRef<string | null | undefined>(undefined);

  useEffect(() => {
    const target = focusAfter.current;
    if (target === undefined) return;
    focusAfter.current = undefined;
    const button =
      target === null
        ? null
        : list.current?.querySelector<HTMLButtonElement>(
            `button[data-entry="${CSS.escape(target)}"]`,
          );
    (button ?? inputRef.current)?.focus();
  }, [entries]);

  useEffect(
    () => () => {
      clearTimeout(undoTimer.current);
    },
    [],
  );

  const remove = (index: number) => {
    const entry = entries[index];
    if (entry === undefined) return;
    focusAfter.current = entries[index + 1] ?? entries[index - 1] ?? null;
    void setEntries(entries.filter((_, i) => i !== index));
    setRemoval({ entry, index });
    clearTimeout(undoTimer.current);
    undoTimer.current = setTimeout(() => {
      setRemoval(null);
    }, UNDO_MS);
  };

  const undo = () => {
    if (removal === null) return;
    clearTimeout(undoTimer.current);
    setRemoval(null);
    // Re-added in the meantime: nothing to restore.
    if (checkEntry(removal.entry, entries).kind !== "ok") return;
    const next = [...entries];
    next.splice(Math.min(removal.index, next.length), 0, removal.entry);
    void setEntries(next);
  };

  const check = checkEntry(input, entries);
  const percent = budgetPercent(entries);
  const nearlyFull = percent >= WARNING_PERCENT;
  const active = models?.active ?? null;
  const activeName = models?.models.find((m) => m.id === active)?.name;

  const add = () => {
    if (check.kind === "duplicate") {
      setDuplicate(check.existing);
      return;
    }
    if (check.kind !== "ok") return;
    void setEntries([...entries, check.entry]);
    setInput("");
  };

  const hintId = "vocabulary-hint";
  const hint =
    duplicate !== null
      ? t("vocabulary.duplicate", { entry: duplicate })
      : check.kind === "tooLong"
        ? t("vocabulary.tooLong", { max: MAX_ENTRY_CHARS })
        : null;

  return (
    <section className="flex flex-col gap-4 rounded-card border border-line bg-surface px-6 py-5">
      <form
        className="flex flex-col gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          add();
        }}
      >
        <label htmlFor="vocabulary-input" className="font-medium">
          {t("vocabulary.addLabel")}
        </label>
        <div className="flex gap-2">
          <input
            ref={inputRef}
            id="vocabulary-input"
            type="text"
            value={input}
            placeholder={t("vocabulary.placeholder")}
            aria-invalid={hint !== null}
            aria-describedby={hint !== null ? hintId : undefined}
            onChange={(event) => {
              setInput(event.target.value);
              setDuplicate(null);
            }}
            className="min-w-0 flex-1 rounded-lg border border-control bg-bg px-3 py-1.5 text-sm placeholder:text-muted"
          />
          <Button type="submit" disabled={check.kind === "empty" || check.kind === "tooLong"}>
            {t("vocabulary.add")}
          </Button>
        </div>
        {hint !== null && (
          <p id={hintId} role="alert" className="text-sm text-danger">
            {hint}
          </p>
        )}
      </form>

      {entries.length === 0 ? (
        <div className="flex items-center gap-3 text-muted">
          <EchoMark className="size-6 shrink-0 text-muted/40" />
          <p className="text-sm">{t("vocabulary.empty")}</p>
        </div>
      ) : (
        <ul ref={list} aria-label={t("vocabulary.listLabel")} className="flex flex-wrap gap-2">
          {entries.map((entry, index) => (
            <li
              key={entry}
              className="flex max-w-full min-w-0 items-center gap-1 rounded-full bg-accent-soft py-1 pr-1 pl-3 text-sm text-accent-soft-fg"
            >
              <span className="min-w-0 break-words">{entry}</span>
              {/* 20px circle, 24px hit area (WCAG 2.5.8) through the pseudo-element. */}
              <button
                type="button"
                data-entry={entry}
                aria-label={t("vocabulary.remove", { entry })}
                onClick={() => {
                  remove(index);
                }}
                className="relative flex size-5 shrink-0 items-center justify-center rounded-full after:absolute after:-inset-0.5 after:content-[''] hover:bg-accent-strong hover:text-accent-fg"
              >
                <svg
                  viewBox="0 0 24 24"
                  width="12"
                  height="12"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  aria-hidden="true"
                >
                  <path d="M6 6l12 12M18 6 6 18" />
                </svg>
              </button>
            </li>
          ))}
        </ul>
      )}

      <div className="flex flex-col gap-1.5">
        <div aria-hidden="true" className="h-1.5 overflow-hidden rounded-full bg-raised">
          <div
            className={`h-full rounded-full ${nearlyFull ? "bg-warning" : "bg-accent"}`}
            style={{ width: `${String(percent)}%` }}
          />
        </div>
        <p className={`text-note ${nearlyFull ? "text-warning" : "text-muted"}`}>
          {t("vocabulary.budget", { used: hintChars(entries), max: HINT_CHAR_BUDGET })}
        </p>
        {nearlyFull && (
          <p role="alert" className="text-sm text-warning">
            {t("vocabulary.nearlyFull")}
          </p>
        )}
      </div>

      {active !== null && activeName !== undefined && !acceptsPrompt(active) && (
        <p
          role="note"
          className="rounded-lg bg-accent-soft px-4 py-3 text-sm leading-relaxed text-accent-soft-fg"
        >
          {t("vocabulary.noPrompt", { model: activeName })}
        </p>
      )}

      {removal !== null && (
        <div className="pointer-events-none fixed inset-x-0 bottom-6 z-10 flex justify-center px-6">
          <div
            role="status"
            className="pointer-events-auto flex max-w-full items-center gap-4 rounded-xl bg-fg px-4 py-2.5 text-sm text-bg shadow-lg"
          >
            <span className="min-w-0 break-words">
              {t("vocabulary.removed", { entry: removal.entry })}
            </span>
            <button
              type="button"
              onClick={undo}
              className="hit-target shrink-0 rounded-md font-medium text-accent-soft underline-offset-4 transition-transform duration-150 ease-out-strong hover:underline active:scale-[0.97] motion-reduce:active:scale-100"
            >
              {t("vocabulary.undo")}
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
