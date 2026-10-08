import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import type { UnloadModelAfter } from "../bindings";
import { ChevronIcon } from "../components/icons";
import { SectionHeading } from "../components/SectionHeading";
import { SettingRow } from "../components/SettingRow";
import { useModels } from "../store/models";
import { useSetting } from "../store/settings";
import { DictationLanguagePicker } from "./DictationLanguagePicker";
import { ModelCard } from "./ModelCard";

/**
 * The "Model & language" page's content (models.md "UI"): the active Model, its language, the
 * memory setting, then the other Models. With no active Model there is a single "Models" list.
 */
export function ModelsSection() {
  const { t } = useTranslation();
  const models = useModels((s) => s.state);
  const load = useModels((s) => s.load);

  useEffect(() => {
    void load();
  }, [load]);

  if (!models) return null;
  const active = models.models.find((m) => m.id === models.active);
  const others = models.models.filter((m) => m.id !== models.active);
  return (
    <div role="region" aria-label={t("models.listLabel")} className="flex flex-col gap-8">
      {models.dictationInProgress && (
        <p role="status" className="text-note text-muted">
          {t("models.busy")}
        </p>
      )}
      {active && (
        // A hairline lavender outline and the lighter surface mark it as the active Model, so it
        // needs no visible heading; it is the one panel on a page of flat rows.
        <section aria-label={t("pages.model.sections.active")}>
          <ul className="rounded-xl border border-accent/45 bg-surface px-5 py-1">
            <ModelCard entry={active} models={models} />
          </ul>
        </section>
      )}
      <DictationLanguagePicker />
      <section aria-labelledby="model-memory-heading">
        <SectionHeading id="model-memory-heading">
          {t("pages.model.sections.memory")}
        </SectionHeading>
        <UnloadAfterPicker />
      </section>
      {others.length > 0 && (
        // Without an active Model this list is the whole "Models" region, so it is not named twice.
        <section aria-labelledby={active ? "other-models-heading" : undefined}>
          <SectionHeading id="other-models-heading">
            {active ? t("pages.model.sections.other") : t("models.listLabel")}
          </SectionHeading>
          <ul>
            {others.map((entry) => (
              <ModelCard key={entry.id} entry={entry} models={models} />
            ))}
          </ul>
        </section>
      )}
    </div>
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
    <SettingRow
      label={t("models.unload.label")}
      description={t("models.unload.description")}
      labelId="unload-after-label"
    >
      {/* A native select for its keyboard and screen-reader behaviour, drawn like the pickers. */}
      <span className="relative shrink-0">
        <select
          value={value}
          aria-labelledby="unload-after-label"
          onChange={(event) => void setValue(event.target.value as UnloadModelAfter)}
          className="min-w-44 cursor-pointer appearance-none rounded-lg border border-line bg-bg py-1.5 pr-9 pl-3 text-sm transition-colors duration-150 hover:bg-raised/50"
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
    </SettingRow>
  );
}
