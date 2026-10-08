import { type ReactNode, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { type DictationProblem, commands } from "../bindings";
import { useDictation } from "../store/dictation";
import { useShell } from "../store/shell";
import { Button } from "./Button";
import { AlertIcon, ArrowIcon, CopyIcon, ExternalIcon, FolderIcon } from "./icons";

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
  // announces the message rather than the buttons around it. It opens the page column, above the
  // title, with a hairline closing it off from the page.
  return (
    <section aria-labelledby="dictation-notices-title" className="border-b border-line pb-2">
      <div className="flex items-center justify-between gap-4">
        <h2
          id="dictation-notices-title"
          className="truncate pb-1 text-sm font-semibold text-danger"
        >
          {t("dictationNotices.title")}
        </h2>
        <button
          type="button"
          onClick={() => void dismiss()}
          className="-mr-2 shrink-0 rounded-md px-2 py-1 text-note font-medium text-muted transition-colors duration-150 hover:bg-raised hover:text-fg"
        >
          {t("dictationNotices.dismiss")}
        </button>
      </div>
      <ul className="flex flex-col">
        {[...notices].reverse().map((notice) => (
          <Notice key={notice.id} notice={notice} kept={keptTranscript === notice.id} />
        ))}
      </ul>
    </section>
  );
}

/**
 * One error as a row: an alert mark, the plain message with the technical detail under it, and
 * what the user can do at the right. Message and detail stay on one line each, cut with an
 * ellipsis and shown whole in the tooltip.
 */
function Notice({ notice, kept }: { notice: DictationProblem; kept: boolean }) {
  const { t } = useTranslation();
  const setPage = useShell((s) => s.setPage);
  const openSection = useShell((s) => s.openSection);
  const message = t(`dictationNotices.kinds.${notice.kind}`);
  const detail = notice.detail ? t("dictationNotices.detail", { detail: notice.detail }) : null;
  return (
    <li className="flex items-center justify-between gap-8 border-b border-line py-3.5 last:border-b-0">
      <div className="flex min-w-0 flex-1 items-start gap-2.5">
        <AlertIcon className="mt-0.5 text-danger" />
        <div className="flex min-w-0 flex-col gap-0.5">
          <p role="alert" title={message} className="truncate font-medium">
            {message}
          </p>
          {detail && (
            <p title={detail} className="cursor-text truncate text-note text-muted select-text">
              {detail}
            </p>
          )}
        </div>
      </div>
      <div className="-mr-3.5 flex shrink-0 items-center">
        {(notice.kind === "noModel" ||
          notice.kind === "modelLoadFailed" ||
          notice.kind === "modelDownloadFailed") && (
          <NoticeAction
            icon={<ArrowIcon />}
            onClick={() => {
              setPage("model");
            }}
          >
            {t("dictationNotices.openModels")}
          </NoticeAction>
        )}
        {notice.kind === "microphoneAccessDenied" && (
          <NoticeAction
            icon={<ExternalIcon />}
            onClick={() => void commands.openMicrophonePrivacySettings()}
          >
            {t("dictationNotices.openPrivacy")}
          </NoticeAction>
        )}
        {(notice.kind === "microphoneNotFound" ||
          notice.kind === "microphoneFailed" ||
          notice.kind === "microphoneDisconnected") && (
          <NoticeAction
            icon={<ArrowIcon />}
            onClick={() => {
              openSection("dictation", "microphone");
            }}
          >
            {t("dictationNotices.openMicrophone")}
          </NoticeAction>
        )}
        {notice.kind === "transcriptionFailed" && (
          <NoticeAction
            icon={<FolderIcon />}
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
            icon={<ArrowIcon />}
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
      <NoticeAction icon={<CopyIcon />} onClick={() => void copy()}>
        <SwapLabel label={label} />
      </NoticeAction>
    </span>
  );
}

/** A quiet icon-and-text action at the row's right, like the App page's "Open log folder". */
function NoticeAction({
  icon,
  onClick,
  children,
}: {
  icon: ReactNode;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <Button variant="quiet" onClick={onClick} className="inline-flex items-center gap-1.5">
      {icon}
      {children}
    </Button>
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
