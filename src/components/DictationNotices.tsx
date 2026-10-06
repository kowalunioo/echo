import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { type DictationProblem, commands } from "../bindings";
import { useDictation } from "../store/dictation";
import { useShell } from "../store/shell";

/**
 * Dictation errors in the main window (dictation-pipeline.md rules 39b–39d): every error since
 * the user last dismissed them, newest first. While the window is visible and focused, the tray's
 * red error icon is cleared.
 */
export function DictationNotices() {
  const { t } = useTranslation();
  const load = useDictation((s) => s.load);
  const notices = useDictation((s) => s.status.notices);
  const hasError = useDictation((s) => s.status.error !== null);
  const seen = useDictation((s) => s.seen);
  const dismiss = useDictation((s) => s.dismiss);

  useEffect(() => {
    void load();
  }, [load]);

  // The user is looking at the window: the tray returns to idle (rule 39c).
  useEffect(() => {
    if (!hasError) return;
    const check = () => {
      if (document.visibilityState === "visible" && document.hasFocus()) void seen();
    };
    check();
    window.addEventListener("focus", check);
    document.addEventListener("visibilitychange", check);
    return () => {
      window.removeEventListener("focus", check);
      document.removeEventListener("visibilitychange", check);
    };
  }, [hasError, seen]);

  if (notices.length === 0) return null;
  return (
    <section
      role="alert"
      aria-labelledby="dictation-notices-title"
      className="mx-auto mt-6 flex max-w-2xl flex-col gap-3 rounded-card border border-danger/40 bg-surface px-6 py-4"
    >
      <div className="flex items-center justify-between gap-4">
        <h2 id="dictation-notices-title" className="font-medium text-danger">
          {t("dictationNotices.title")}
        </h2>
        <button
          type="button"
          onClick={() => void dismiss()}
          className="rounded-lg px-3 py-1 text-sm text-muted hover:bg-bg hover:text-fg focus-visible:outline-2 focus-visible:outline-focus"
        >
          {t("dictationNotices.dismiss")}
        </button>
      </div>
      <ul className="flex flex-col gap-2">
        {[...notices].reverse().map((notice) => (
          <Notice key={notice.id} notice={notice} />
        ))}
      </ul>
    </section>
  );
}

function Notice({ notice }: { notice: DictationProblem }) {
  const { t } = useTranslation();
  const setPage = useShell((s) => s.setPage);
  return (
    <li className="flex items-start justify-between gap-4 text-sm">
      <div className="flex flex-col gap-0.5">
        <span>{t(`dictationNotices.kinds.${notice.kind}`)}</span>
        {notice.detail && (
          <span className="text-xs break-words text-muted">
            {t("dictationNotices.detail", { detail: notice.detail })}
          </span>
        )}
      </div>
      {notice.kind === "noModel" && (
        <NoticeAction
          onClick={() => {
            setPage("model");
          }}
        >
          {t("dictationNotices.openModels")}
        </NoticeAction>
      )}
      {notice.kind === "microphoneAccessDenied" && (
        <NoticeAction onClick={() => void commands.openMicrophonePrivacySettings()}>
          {t("dictationNotices.openPrivacy")}
        </NoticeAction>
      )}
    </li>
  );
}

function NoticeAction({ onClick, children }: { onClick: () => void; children: string }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="shrink-0 rounded-lg border border-line px-3 py-1 text-sm font-medium text-accent-strong hover:bg-bg focus-visible:outline-2 focus-visible:outline-focus"
    >
      {children}
    </button>
  );
}
