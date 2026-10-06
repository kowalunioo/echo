import { useState } from "react";
import { useTranslation } from "react-i18next";

import { useModels } from "../store/models";
import { useSetting } from "../store/settings";
import {
  MAX_ENTRY_CHARS,
  WARNING_PERCENT,
  acceptsPrompt,
  budgetPercent,
  checkEntry,
} from "./vocabulary";

/** The Vocabulary page: add field, entries as removable chips, hint budget (vocabulary.md "UI"). */
export function VocabularySection() {
  const { t } = useTranslation();
  const [entries, setEntries] = useSetting("vocabulary");
  const models = useModels((s) => s.state);
  const [input, setInput] = useState("");
  const [duplicate, setDuplicate] = useState<string | null>(null);

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
            className="min-w-0 flex-1 rounded-lg border border-line bg-bg px-3 py-1.5 text-sm"
          />
          <button
            type="submit"
            disabled={check.kind === "empty" || check.kind === "tooLong"}
            className="shrink-0 rounded-lg bg-accent-strong px-4 py-1.5 text-sm font-medium text-accent-fg disabled:opacity-50"
          >
            {t("vocabulary.add")}
          </button>
        </div>
        {hint !== null && (
          <p id={hintId} role="alert" className="text-sm text-danger">
            {hint}
          </p>
        )}
      </form>

      {entries.length === 0 ? (
        <p className="text-sm text-muted">{t("vocabulary.empty")}</p>
      ) : (
        <ul aria-label={t("vocabulary.listLabel")} className="flex flex-wrap gap-2">
          {entries.map((entry) => (
            <li
              key={entry}
              className="flex items-center gap-1 rounded-full bg-accent-soft py-1 pr-1 pl-3 text-sm text-accent-soft-fg"
            >
              <span>{entry}</span>
              <button
                type="button"
                aria-label={t("vocabulary.remove", { entry })}
                onClick={() => void setEntries(entries.filter((e) => e !== entry))}
                className="flex size-5 items-center justify-center rounded-full hover:bg-accent-strong hover:text-accent-fg"
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
            className={`h-full rounded-full ${nearlyFull ? "bg-danger" : "bg-accent"}`}
            style={{ width: `${String(percent)}%` }}
          />
        </div>
        <p className={`text-sm ${nearlyFull ? "text-danger" : "text-muted"}`}>
          {t("vocabulary.budget", { percent })}
        </p>
        {nearlyFull && (
          <p role="alert" className="text-sm text-danger">
            {t("vocabulary.nearlyFull")}
          </p>
        )}
      </div>

      {active !== null && activeName !== undefined && !acceptsPrompt(active) && (
        <p role="note" className="text-sm text-muted">
          {t("vocabulary.noPrompt", { model: activeName })}
        </p>
      )}
    </section>
  );
}
