import { HistoryPage } from "../history/HistoryPage";
import type { Page } from "../store/shell";
import { NoModelPanel } from "../models/ModelIndicator";
import { ModelsSection } from "../models/ModelsSection";
import { AppSettings } from "./AppSettings";
import { DictationNotices } from "./DictationNotices";
import { UpdatedNotice } from "./UpdatedNotice";
import { PageHeader } from "./PageHeader";
import { MicrophoneSettings } from "./MicrophoneSettings";
import { RecordShortcutSettings } from "./RecordShortcutSettings";
import { VocabularySection } from "../vocabulary/VocabularySection";

export function PageView({ page }: { page: Page }) {
  return (
    <article className="flex max-w-[760px] flex-col gap-8 px-10 py-10">
      {/* Notices open the column, above the page's own title, on every page. */}
      <UpdatedNotice />
      <DictationNotices />
      <PageHeader page={page} />

      {page !== "history" && <NoModelPanel onModelsPage={page === "model"} />}

      {page === "model" && <ModelsSection />}
      {page === "dictation" && <RecordShortcutSettings />}
      {page === "dictation" && <MicrophoneSettings />}
      {page === "app" && <AppSettings />}
      {page === "vocabulary" && <VocabularySection />}
      {page === "history" && <HistoryPage />}
    </article>
  );
}
