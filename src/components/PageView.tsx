import { HistoryPage } from "../history/HistoryPage";
import type { Page } from "../store/shell";
import { NoModelPanel } from "../models/ModelIndicator";
import { ModelsSection } from "../models/ModelsSection";
import { AppSettings } from "./AppSettings";
import { PageHeader } from "./PageHeader";
import { MicrophoneSettings } from "./MicrophoneSettings";
import { RecordShortcutSettings } from "./RecordShortcutSettings";
import { VocabularySection } from "../vocabulary/VocabularySection";

export function PageView({ page }: { page: Page }) {
  if (page === "history") return <HistoryPage />;
  return (
    <article className="mx-auto flex max-w-2xl flex-col gap-6 px-10 py-12">
      <PageHeader page={page} />

      <NoModelPanel onModelsPage={page === "model"} />

      {page === "model" && <ModelsSection />}
      {page === "dictation" && <RecordShortcutSettings />}
      {page === "dictation" && <MicrophoneSettings />}
      {page === "app" && <AppSettings />}
      {page === "vocabulary" && <VocabularySection />}
    </article>
  );
}
