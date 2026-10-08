import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { ModelIndicator } from "../models/ModelIndicator";
import { useRecordShortcut } from "../store/recordShortcut";
import { PAGES, useShell } from "../store/shell";
import { DictationStatusCard } from "./DictationStatusCard";
import { PageIcon } from "./icons";
import { UpdateIndicator } from "./UpdateIndicator";

/** Ctrl+1 … Ctrl+5 open the sections in PAGES order. */
function sectionKeys(index: number) {
  return `Control+${String(index + 1)}`;
}

/** Whether the key goes to a text field (or a capture), so it must not switch sections. */
function typing(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}

/** Ctrl+1 … Ctrl+5 switch sections in the main window, except while typing or capturing. */
function useSectionShortcuts(setPage: (page: (typeof PAGES)[number]) => void) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!event.ctrlKey || event.altKey || event.shiftKey || event.metaKey || event.repeat) return;
      if (typing(event.target) || useRecordShortcut.getState().capture) return;
      const page = PAGES[Number(event.key) - 1];
      if (!/^[1-9]$/.test(event.key) || page === undefined) return;
      event.preventDefault();
      setPage(page);
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [setPage]);
}

export function Sidebar() {
  const { t } = useTranslation();
  const page = useShell((s) => s.page);
  const setPage = useShell((s) => s.setPage);
  useSectionShortcuts(setPage);

  return (
    <aside className="flex w-68 shrink-0 flex-col border-r border-line bg-sidebar px-2.5 pt-2 pb-4">
      <nav aria-label={t("nav.label")}>
        <ul className="flex flex-col gap-0.5">
          {PAGES.map((id, index) => {
            const current = id === page;
            return (
              <li key={id}>
                <button
                  type="button"
                  onClick={() => {
                    setPage(id);
                  }}
                  aria-current={current ? "page" : undefined}
                  aria-keyshortcuts={sectionKeys(index)}
                  title={t("nav.shortcut", { keys: `Ctrl+${String(index + 1)}` })}
                  className={`flex w-full items-center gap-2.5 rounded-[9px] px-2.5 py-2 text-left text-[13.5px] transition-colors duration-150 ${
                    current
                      ? "bg-raised font-medium text-fg"
                      : "text-muted hover:bg-surface hover:text-fg"
                  }`}
                >
                  <span className={current ? "text-accent" : ""}>
                    <PageIcon page={id} />
                  </span>
                  {t(`nav.${id}`)}
                </button>
              </li>
            );
          })}
        </ul>
      </nav>

      <StatusArea />
    </aside>
  );
}

/**
 * The Dictation state with the Record Shortcut, the active Model state
 * (settings-and-first-run.md "UI") and the updates row (updater.md "UI").
 */
function StatusArea() {
  const { t } = useTranslation();
  return (
    <section aria-label={t("status.label")} className="mt-auto flex flex-col gap-0.5">
      <DictationStatusCard />
      <ModelIndicator />
      <UpdateIndicator />
    </section>
  );
}
