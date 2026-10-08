import { type ReactNode, useEffect, useRef, useState } from "react";
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
  const keptTranscript = useDictation((s) => s.status.keptTranscript);
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
  // The section is a labelled region; each message line is the alert, so a screen reader
  // announces the message rather than the buttons around it.
  return (
    <section
      aria-labelledby="dictation-notices-title"
      className="mx-auto mt-6 flex max-w-2xl flex-col gap-3 rounded-card border border-danger/40 bg-surface px-6 py-4"
    >
      <div className="flex items-center justify-between gap-4">
        <h2 id="dictation-notices-title" className="text-heading text-danger">
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
      <ul className="flex flex-col gap-3">
        {[...notices].reverse().map((notice) => (
          <Notice key={notice.id} notice={notice} kept={keptTranscript === notice.id} />
        ))}
      </ul>
    </section>
  );
}

/** One error: a plain message, the technical detail below it, and what the user can do. */
function Notice({ notice, kept }: { notice: DictationProblem; kept: boolean }) {
  const { t } = useTranslation();
  const setPage = useShell((s) => s.setPage);
  const openSection = useShell((s) => s.openSection);
  return (
    <li className="flex items-start justify-between gap-4 text-sm">
      <div className="flex min-w-0 flex-col gap-0.5">
        <p role="alert">{t(`dictationNotices.kinds.${notice.kind}`)}</p>
        {notice.detail && (
          <p className="cursor-text text-note break-words text-muted select-text">
            {t("dictationNotices.detail", { detail: notice.detail })}
          </p>
        )}
      </div>
      <div className="flex shrink-0 flex-wrap justify-end gap-2">
        {(notice.kind === "noModel" ||
          notice.kind === "modelLoadFailed" ||
          notice.kind === "modelDownloadFailed") && (
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
        {(notice.kind === "microphoneNotFound" ||
          notice.kind === "microphoneFailed" ||
          notice.kind === "microphoneDisconnected") && (
          <NoticeAction
            onClick={() => {
              openSection("dictation", "microphone");
            }}
          >
            {t("dictationNotices.openMicrophone")}
          </NoticeAction>
        )}
        {notice.kind === "transcriptionFailed" && (
          <NoticeAction
            onClick={() => {
              commands.openLogFolder().catch((error: unknown) => {
                console.error("open_log_folder failed", error);
              });
            }}
          >
            {t("dictationNotices.openLogFolder")}
          </NoticeAction>
        )}
        {kept && <CopyKeptTranscript />}
        {notice.kind === "insertionFailed" && (
          <NoticeAction
            onClick={() => {
              setPage("history");
            }}
          >
            {t("dictationNotices.openHistory")}
          </NoticeAction>
        )}
      </div>
    </li>
  );
}

/** How long "Copied" replaces the button's label. */
const COPIED_MS = 2_000;

/**
 * "Copy text" for a failed Insertion (dictation-pipeline.md rule 36): copies the Transcript Echo
 * holds until the next Recording starts, whatever the History limit.
 */
function CopyKeptTranscript() {
  const { t } = useTranslation();
  const [result, setResult] = useState<"copied" | "failed" | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(
    () => () => {
      clearTimeout(timer.current);
    },
    [],
  );

  const copy = async () => {
    clearTimeout(timer.current);
    try {
      const text = await commands.getKeptTranscript();
      if (text === null) throw new Error("no Transcript is kept any more");
      await navigator.clipboard.writeText(text);
      setResult("copied");
    } catch (error) {
      console.error("copying the kept Transcript failed", error);
      setResult("failed");
    }
    timer.current = setTimeout(() => {
      setResult(null);
    }, COPIED_MS);
  };

  const label = {
    copied: t("dictationNotices.copied"),
    failed: t("dictationNotices.copyFailed"),
    none: t("dictationNotices.copyText"),
  }[result ?? "none"];
  return (
    <span aria-live="polite" className="contents">
      <NoticeAction onClick={() => void copy()}>
        <SwapLabel label={label} />
      </NoticeAction>
    </span>
  );
}

function NoticeAction({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="shrink-0 rounded-lg border border-control px-3 py-1 text-sm font-medium text-accent-strong transition-transform duration-150 ease-out-strong hover:bg-bg focus-visible:outline-2 focus-visible:outline-focus active:scale-[0.97] motion-reduce:active:scale-100"
    >
      {children}
    </button>
  );
}

/** A label that dips to a blur for 200 ms whenever its text changes. */
function SwapLabel({ label }: { label: string }) {
  const [swapping, setSwapping] = useState(false);
  const first = useRef(true);
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    setSwapping(true);
    const timer = setTimeout(() => {
      setSwapping(false);
    }, 200);
    return () => {
      clearTimeout(timer);
    };
  }, [label]);
  return (
    <span
      className={`inline-block transition-[filter,opacity] duration-200 ease-out-strong ${
        swapping ? "opacity-70 blur-[2px] motion-reduce:blur-none" : ""
      }`}
    >
      {label}
    </span>
  );
}
