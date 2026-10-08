import { useEffect, useId, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { commands, type ModelLanguages } from "../bindings";
import { ChevronIcon } from "../components/icons";
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
        <p className="mt-0.5 text-note text-muted">{t("dictationLanguage.detectsItself")}</p>
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
      <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3">
        <span className="flex min-w-0 grow basis-48 flex-col gap-0.5 break-words">
          <h3 className="font-medium">{title}</h3>
          <span className="text-note text-muted">{t("dictationLanguage.description")}</span>
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
        <p role="note" className="text-note text-muted">
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
          className="hit-target self-start rounded-md text-sm font-medium text-accent-strong underline-offset-4 hover:underline"
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

/**
 * A button that opens a searchable list. Typing filters and makes the first match active; the
 * arrow keys, Home and End move the active option, Enter picks it, Escape closes.
 */
function LanguageCombobox({ title, options, value, label, onPick }: ComboboxProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
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

  const show = () => {
    setQuery("");
    setActive(Math.max(0, options.indexOf(value)));
    setOpen(true);
  };
  const optionId = (index: number) => `${listId}-option-${String(index)}`;
  const activeIndex = Math.min(active, matches.length - 1);

  // Keeps the active option in view while the arrow keys move it through a long list.
  useEffect(() => {
    if (!open || activeIndex < 0) return;
    const option = document.getElementById(`${listId}-option-${String(activeIndex)}`);
    // jsdom has no scrollIntoView.
    if (typeof option?.scrollIntoView === "function") option.scrollIntoView({ block: "nearest" });
  }, [open, activeIndex, listId]);

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
          if (open) setOpen(false);
          else show();
        }}
        onKeyDown={(event) => {
          if (!open && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
            event.preventDefault();
            show();
          }
        }}
        className="flex max-w-64 min-w-48 items-center justify-between gap-3 rounded-lg border border-control bg-bg px-3 py-1.5 text-sm"
      >
        <span className="truncate" title={label(value)}>
          {label(value)}
        </span>
        <ChevronIcon />
      </button>
      {open && (
        <div className="absolute right-0 z-10 mt-1 flex w-64 flex-col gap-1 rounded-lg border border-line bg-surface p-2 shadow-lg">
          <input
            type="search"
            autoFocus
            value={query}
            aria-label={t("dictationLanguage.search")}
            aria-controls={listId}
            aria-activedescendant={activeIndex >= 0 ? optionId(activeIndex) : undefined}
            placeholder={t("dictationLanguage.search")}
            onChange={(event) => {
              setQuery(event.target.value);
              setActive(0);
            }}
            onKeyDown={(event) => {
              const last = matches.length - 1;
              const moves: Record<string, () => number> = {
                ArrowDown: () => Math.min(last, activeIndex + 1),
                ArrowUp: () => Math.max(0, activeIndex - 1),
                Home: () => 0,
                End: () => last,
              };
              const move = moves[event.key];
              if (move) {
                event.preventDefault();
                setActive(Math.max(0, move()));
              } else if (event.key === "Enter") {
                event.preventDefault();
                const option = matches[activeIndex];
                if (option !== undefined) pick(option);
              } else if (event.key === "Escape") {
                event.preventDefault();
                event.stopPropagation();
                close();
              } else if (event.key === "Tab") {
                setOpen(false);
              }
            }}
            className="rounded-lg border border-control bg-bg px-3 py-1.5 text-sm placeholder:text-muted"
          />
          <ul id={listId} role="listbox" aria-label={title} className="max-h-64 overflow-y-auto">
            {matches.map((option, index) => (
              <li
                key={option}
                id={optionId(index)}
                role="option"
                aria-selected={option === value}
                onPointerMove={() => {
                  setActive(index);
                }}
                onClick={() => {
                  pick(option);
                }}
                className={`cursor-pointer rounded-md px-2 py-1 text-sm ${
                  index === activeIndex ? "bg-raised" : ""
                } ${option === value ? "font-medium text-accent-soft-fg" : ""} ${
                  option === value && index !== activeIndex ? "bg-accent-soft" : ""
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
