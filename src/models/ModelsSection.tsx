import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import type { UnloadModelAfter } from "../bindings";
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
    <label className="flex items-center justify-between gap-8 rounded-card border border-line bg-surface px-6 py-4">
      <span className="flex flex-col gap-0.5">
        <span className="font-medium">{t("models.unload.label")}</span>
        <span className="text-xs text-muted">{t("models.unload.description")}</span>
      </span>
      <select
        value={value}
        onChange={(event) => void setValue(event.target.value as UnloadModelAfter)}
        className="shrink-0 rounded-lg border border-line bg-bg px-3 py-1.5 text-sm"
      >
        {UNLOAD_CHOICES.map(({ value: choice, minutes }) => (
          <option key={choice} value={choice}>
            {minutes === null
              ? t("models.unload.never")
              : t("models.unload.minutes", { count: minutes })}
          </option>
        ))}
      </select>
    </label>
  );
}
