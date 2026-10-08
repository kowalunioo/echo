import { useEffect, useId, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { commands, type ModelLanguages } from "../bindings";
import { IconButton } from "../components/IconButton";
import { ChevronIcon, ResetIcon } from "../components/icons";
import { SectionHeading } from "../components/SectionHeading";
import { SettingRow } from "../components/SettingRow";
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
  const heading = (
    <SectionHeading id="dictation-language-heading">
      {t("pages.model.sections.language")}
    </SectionHeading>
  );
  if (!support.honoursLanguage) {
    return (
      <section aria-label={title}>
        {heading}
        <SettingRow label={title} description={t("dictationLanguage.detectsItself")} />
      </section>
    );
  }

  const label = (value: string) =>
    value === AUTOMATIC ? t("dictationLanguage.automatic") : languageName(value, uiLanguage);
  const effective = resolveLanguage(intent, support);
  const unavailable = intent !== AUTOMATIC && findLanguage(intent, support.languages) === null;

  return (
    // Named after the Model, so a screen reader hears which Model the language belongs to.
    <section aria-label={title}>
      {heading}
      <SettingRow
        label={title}
        description={t("dictationLanguage.description")}
        below={
          unavailable && (
            <p role="note" className="text-note text-muted">
              {t("dictationLanguage.unavailable", {
                language: label(intent),
                fallback: label(effective),
              })}
            </p>
          )
        }
      >
        <LanguageCombobox
          title={title}
          options={languageOptions(support, uiLanguage)}
          value={effective}
          label={label}
          onPick={(value) => void setIntent(value)}
        />
        {/* Only shown once the language differs from Automatic, at the row's right edge. */}
        {intent !== AUTOMATIC && (
          <IconButton
            label={t("dictationLanguage.reset")}
            onClick={() => void useSettings.getState().reset("dictationLanguage")}
            className="-mr-1.5"
          >
            <ResetIcon />
          </IconButton>
        )}
      </SettingRow>
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
        className="flex max-w-56 min-w-44 items-center justify-between gap-3 rounded-lg border border-line bg-bg px-3 py-1.5 text-sm transition-colors duration-150 hover:bg-raised/50"
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
            className="rounded-lg border border-line bg-bg px-3 py-1.5 text-sm placeholder:text-muted"
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
                } ${option === value ? "font-medium" : ""} ${
                  option === value && index !== activeIndex ? "bg-raised/50" : ""
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
