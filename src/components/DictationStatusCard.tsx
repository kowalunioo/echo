import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import type { DictationStatus } from "../bindings";
import { formatElapsed } from "../overlay/meter";
import { keysOf } from "../shortcut/capture";
import { useKeyLabel } from "../shortcut/keyLabel";
import { useDictation } from "../store/dictation";
import { DEFAULT_RECORD_SHORTCUT } from "../store/recordShortcut";
import { useSettings } from "../store/settings";
import { Keycap } from "./Keycap";

type Phase = "idle" | "gettingReady" | "listening" | "transcribing" | "inserting" | "failed";

/** What the Dictation is doing, as the user would put it (PRODUCT.md principle 4). */
function phaseOf(status: DictationStatus): Phase {
  switch (status.state) {
    case "recording":
      return status.listening ? "listening" : "gettingReady";
    case "transcribing":
      return "transcribing";
    case "inserting":
      return "inserting";
    case "idle":
      // The notices stay until dismissed, so the sidebar agrees with the error panel.
      return status.notices.length > 0 ? "failed" : "idle";
  }
}

const DOT: Record<Phase, string> = {
  idle: "bg-muted/50",
  gettingReady: "bg-muted animate-pulse",
  listening: "bg-accent",
  transcribing: "bg-accent animate-pulse",
  inserting: "bg-accent animate-pulse",
  failed: "bg-danger",
};

/**
 * The live Dictation state and the Record Shortcut in the sidebar. It follows the dictation
 * status itself, so it is honest whether or not the Overlay is shown.
 */
export function DictationStatusCard() {
  const { t } = useTranslation();
  const load = useDictation((s) => s.load);
  const status = useDictation((s) => s.status);
  const phase = phaseOf(status);
  const elapsedMs = useRecordingTimer(status.state === "recording");

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="flex flex-col gap-2 rounded-lg border border-line bg-surface px-3 py-2.5">
      <div className="flex items-center gap-2.5">
        <span className={`size-2 shrink-0 rounded-full ${DOT[phase]}`} aria-hidden="true" />
        <span
          role="status"
          className={`min-w-0 flex-1 text-note font-medium ${phase === "failed" ? "text-danger" : ""}`}
        >
          {t(`status.dictation.${phase}`)}
        </span>
        {phase === "listening" && (
          <span data-testid="status-timer" className="text-note text-muted tabular-nums">
            {formatElapsed(elapsedMs)}
          </span>
        )}
      </div>
      <ShortcutHint />
    </div>
  );
}

/** "Hold Ctrl + Space": how to start a Dictation, in the current Shortcut mode. */
function ShortcutHint() {
  const { t } = useTranslation();
  const label = useKeyLabel();
  const combination = useSettings((s) => s.settings?.recordShortcut ?? DEFAULT_RECORD_SHORTCUT);
  const mode = useSettings((s) => s.settings?.shortcutMode ?? "pushToTalk");

  return (
    <div
      role="group"
      data-testid="status-shortcut"
      aria-label={t(`status.shortcut.label_${mode}`, { shortcut: label(combination) })}
      title={t(`recordShortcut.mode.${mode}`)}
      className="flex flex-wrap items-center gap-1 text-note text-muted"
    >
      <span aria-hidden="true" className="mr-0.5">
        {t(`status.shortcut.${mode}`)}
      </span>
      {keysOf(combination).map((key, i) => (
        <span key={key} aria-hidden="true" className="flex items-center gap-1">
          {i > 0 && <span>+</span>}
          <Keycap size="small">{label(key)}</Keycap>
        </span>
      ))}
    </div>
  );
}

/** Time since the Recording started, ticking each second; a new Recording starts from zero. */
function useRecordingTimer(recording: boolean): number {
  const [elapsedMs, setElapsedMs] = useState(0);
  const [wasRecording, setWasRecording] = useState(recording);
  if (recording !== wasRecording) {
    setWasRecording(recording);
    if (recording) setElapsedMs(0);
  }
  useEffect(() => {
    if (!recording) return;
    const start = Date.now();
    const timer = setInterval(() => {
      setElapsedMs(Date.now() - start);
    }, 1000);
    return () => {
      clearInterval(timer);
    };
  }, [recording]);
  return elapsedMs;
}
