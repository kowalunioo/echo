import { useEffect, useId, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { commands, type ModelLanguages } from "../bindings";
import { useModels } from "../store/models";
import { useSetting, useSettings } from "../store/settings";
import {
  AUTOMATIC,
  findLanguage,
  languageName,
  languageOptions,
  resolveLanguage,
  searchKey,
} from "./dictationLanguage";

/**
 * The Dictation Language picker with the active Model's settings (dictation-language.md "UI").
 * It shows the effective language for the active Model; the stored intent is never rewritten.
 */
export function DictationLanguagePicker() {
  const { t } = useTranslation();
  const models = useModels((s) => s.state);
  const [intent, setIntent] = useSetting("dictationLanguage");
  const [uiLanguage] = useSetting("uiLanguage");
  const [all, setAll] = useState<ModelLanguages[] | null>(null);

  useEffect(() => {
    let live = true;
    void commands.getModelLanguages().then((languages) => {
      if (live) setAll(languages);
    });
    return () => {
      live = false;
    };
  }, []);

  const active = models?.active ?? null;
  const support = all?.find((m) => m.model === active);
  const modelName = models?.models.find((m) => m.id === active)?.name;
  if (!support || !modelName) return null;

  const title = t("dictationLanguage.title", { model: modelName });
  if (!support.honoursLanguage) {
    return (
      <section aria-label={title} className="rounded-card border border-line bg-surface px-6 py-4">
        <h3 className="font-medium">{title}</h3>
        <p className="mt-0.5 text-sm text-muted">{t("dictationLanguage.detectsItself")}</p>
      </section>
    );
  }

  const label = (value: string) =>
    value === AUTOMATIC ? t("dictationLanguage.automatic") : languageName(value, uiLanguage);
  const effective = resolveLanguage(intent, support);
  const unavailable = intent !== AUTOMATIC && findLanguage(intent, support.languages) === null;

  return (
    <section
      aria-label={title}
      className="flex flex-col gap-3 rounded-card border border-line bg-surface px-6 py-4"
    >
      <div className="flex items-center justify-between gap-8">
        <span className="flex flex-col gap-0.5">
          <h3 className="font-medium">{title}</h3>
          <span className="text-xs text-muted">{t("dictationLanguage.description")}</span>
        </span>
        <LanguageCombobox
          title={title}
          options={languageOptions(support, uiLanguage)}
          value={effective}
          label={label}
          onPick={(value) => void setIntent(value)}
        />
      </div>
      {unavailable && (
        <p role="note" className="text-sm text-muted">
          {t("dictationLanguage.unavailable", {
            language: label(intent),
            fallback: label(effective),
          })}
        </p>
      )}
      {intent !== AUTOMATIC && (
        <button
          type="button"
          onClick={() => void useSettings.getState().reset("dictationLanguage")}
          className="self-start text-sm font-medium text-accent-strong hover:underline"
        >
          {t("dictationLanguage.reset")}
        </button>
      )}
    </section>
  );
}

interface ComboboxProps {
  title: string;
  options: string[];
  value: string;
  label: (value: string) => string;
  onPick: (value: string) => void;
}

/** A button that opens a searchable list: typing filters, Enter picks the first match, Escape closes. */
function LanguageCombobox({ title, options, value, label, onPick }: ComboboxProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const listId = useId();

  const matches = useMemo(() => {
    const wanted = searchKey(query.trim());
    return options.filter((option) => searchKey(label(option)).includes(wanted));
  }, [options, query, label]);

  useEffect(() => {
    if (!open) return;
    const outside = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", outside);
    return () => {
      document.removeEventListener("mousedown", outside);
    };
  }, [open]);

  const close = () => {
    setOpen(false);
    button.current?.focus();
  };
  const pick = (option: string) => {
    onPick(option);
    close();
  };

  return (
    <div ref={root} className="relative shrink-0">
      <button
        ref={button}
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={`${title}: ${label(value)}`}
        onClick={() => {
          setQuery("");
          setOpen(!open);
        }}
        className="flex min-w-48 items-center justify-between gap-3 rounded-lg border border-line bg-bg px-3 py-1.5 text-sm"
      >
        <span>{label(value)}</span>
        <span aria-hidden="true" className="text-muted">
          ▾
        </span>
      </button>
      {open && (
        <div className="absolute right-0 z-10 mt-1 flex w-64 flex-col gap-1 rounded-lg border border-line bg-surface p-2 shadow-lg">
          <input
            type="search"
            autoFocus
            value={query}
            aria-label={t("dictationLanguage.search")}
            aria-controls={listId}
            placeholder={t("dictationLanguage.search")}
            onChange={(event) => {
              setQuery(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                const first = matches[0];
                if (first !== undefined) pick(first);
              } else if (event.key === "Escape") {
                event.preventDefault();
                event.stopPropagation();
                close();
              }
            }}
            className="rounded-md border border-line bg-bg px-2 py-1 text-sm"
          />
          <ul id={listId} role="listbox" aria-label={title} className="max-h-64 overflow-y-auto">
            {matches.map((option) => (
              <li
                key={option}
                role="option"
                aria-selected={option === value}
                onClick={() => {
                  pick(option);
                }}
                className={`cursor-pointer rounded-md px-2 py-1 text-sm hover:bg-accent-soft ${
                  option === value ? "bg-accent-soft font-medium text-accent-soft-fg" : ""
                }`}
              >
                {label(option)}
              </li>
            ))}
            {matches.length === 0 && (
              <li className="px-2 py-1 text-sm text-muted">{t("dictationLanguage.noMatch")}</li>
            )}
          </ul>
        </div>
      )}
    </div>
  );
}
