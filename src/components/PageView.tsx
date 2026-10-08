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
    <article className="flex max-w-[760px] flex-col gap-8 px-10 py-10">
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
