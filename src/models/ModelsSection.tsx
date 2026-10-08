import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import type { UnloadModelAfter } from "../bindings";
import { ChevronIcon } from "../components/icons";
import { useModels } from "../store/models";
import { useSetting } from "../store/settings";
import { DictationLanguagePicker } from "./DictationLanguagePicker";
import { ModelCard } from "./ModelCard";

/** The Models part of the "Model & language" page (models.md "UI"). */
export function ModelsSection() {
  const { t } = useTranslation();
  const models = useModels((s) => s.state);
  const load = useModels((s) => s.load);

  useEffect(() => {
    void load();
  }, [load]);

  if (!models) return null;
  return (
    <section aria-label={t("models.listLabel")} className="flex flex-col gap-4">
      {models.dictationInProgress && (
        <p role="status" className="text-sm text-muted">
          {t("models.busy")}
        </p>
      )}
      <ul className="flex flex-col gap-3">
        {models.models.map((entry) => (
          <ModelCard key={entry.id} entry={entry} models={models} />
        ))}
      </ul>
      <DictationLanguagePicker />
      <UnloadAfterPicker />
    </section>
  );
}

const UNLOAD_CHOICES: { value: UnloadModelAfter; minutes: number | null }[] = [
  { value: "never", minutes: null },
  { value: "minutes2", minutes: 2 },
  { value: "minutes5", minutes: 5 },
  { value: "minutes10", minutes: 10 },
  { value: "minutes15", minutes: 15 },
  { value: "minutes60", minutes: 60 },
];

function UnloadAfterPicker() {
  const { t } = useTranslation();
  const [value, setValue] = useSetting("unloadModelAfter");
  return (
    <label className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3 rounded-card border border-line bg-surface px-6 py-4">
      <span className="flex min-w-0 grow basis-48 flex-col gap-0.5 break-words">
        <span className="font-medium">{t("models.unload.label")}</span>
        <span className="text-note text-muted">{t("models.unload.description")}</span>
      </span>
      {/* A native select for its keyboard and screen-reader behaviour, drawn like the pickers. */}
      <span className="relative shrink-0">
        <select
          value={value}
          onChange={(event) => void setValue(event.target.value as UnloadModelAfter)}
          className="min-w-48 cursor-pointer appearance-none rounded-lg border border-control bg-bg py-1.5 pr-9 pl-3 text-sm transition-colors duration-150 hover:border-muted"
        >
          {UNLOAD_CHOICES.map(({ value: choice, minutes }) => (
            <option key={choice} value={choice}>
              {minutes === null
                ? t("models.unload.never")
                : t("models.unload.minutes", { count: minutes })}
            </option>
          ))}
        </select>
        <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center">
          <ChevronIcon />
        </span>
      </span>
    </label>
  );
}
