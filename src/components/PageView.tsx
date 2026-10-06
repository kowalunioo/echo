import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { HistoryPage } from "../history/HistoryPage";
import type { Page } from "../store/shell";
import { NoModelPanel } from "../models/ModelIndicator";
import { ModelsSection } from "../models/ModelsSection";
import { AppSettings } from "./AppSettings";
import { PageHeader } from "./PageHeader";
import { MicrophoneSettings } from "./MicrophoneSettings";
import { RecordShortcutSettings } from "./RecordShortcutSettings";

export function PageView({ page }: { page: Page }) {
  const { t } = useTranslation();
  if (page === "history") return <HistoryPage />;
  return (
    <article className="mx-auto flex max-w-2xl flex-col gap-6 px-10 py-12">
      <PageHeader page={page} />

      <NoModelPanel onModelsPage={page === "model"} />

      {page === "model" && <ModelsSection />}
      {page === "dictation" && <RecordShortcutSettings />}
      {page === "dictation" && <MicrophoneSettings />}
      {page === "app" && <AppSettings />}

      <Placeholder>{t(`pages.${page}.upcoming`)}</Placeholder>
    </article>
  );
}

export function Placeholder({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  return (
    <section className="flex flex-col items-start gap-3 rounded-card border border-dashed border-line px-6 py-5">
      <span className="rounded-full bg-accent-soft px-2.5 py-0.5 text-xs font-medium text-accent-soft-fg">
        {t("placeholder.badge")}
      </span>
      <p className="text-muted">
        {t("placeholder.lead")} <span className="text-fg">{children}</span>
      </p>
    </section>
  );
}
