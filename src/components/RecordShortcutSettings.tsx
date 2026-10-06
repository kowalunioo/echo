import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import { type ShortcutMode, events } from "../bindings";
import { heldCombination, keysOf } from "../shortcut/capture";
import {
  DEFAULT_RECORD_SHORTCUT,
  type ShortcutFeedback,
  useRecordShortcut,
} from "../store/recordShortcut";
import { useSetting } from "../store/settings";

const MODES: readonly ShortcutMode[] = ["pushToTalk", "toggle"];

/** Record Shortcut and its mode on the Dictation page (record-shortcut.md, "UI"). */
export function RecordShortcutSettings() {
  const { t } = useTranslation();
  const captureKey = useRecordShortcut((s) => s.captureKey);
  const showIntent = useRecordShortcut((s) => s.showIntent);
  const feedback = useRecordShortcut((s) => s.feedback);

  useEffect(() => {
    const subscriptions = [
      events.capturedKeyEvent.listen((e) => void captureKey(e.payload.key, e.payload.pressed)),
      events.recordIntentEvent.listen((e) => {
        showIntent(e.payload.intent);
      }),
    ].map((subscription) =>
      subscription.catch((error: unknown) => {
        console.error("cannot listen for Record Shortcut events", error);
        return null;
      }),
    );
    return () => {
      for (const subscription of subscriptions) {
        void subscription.then((unlisten) => unlisten?.());
      }
    };
  }, [captureKey, showIntent]);

  return (
    <>
      <section className="divide-y divide-line rounded-card border border-line bg-surface">
        <div className="flex flex-col gap-2 px-6 py-4">
          <div className="flex items-center justify-between gap-8">
            <div className="flex flex-col gap-0.5">
              <span className="font-medium" id="record-shortcut-label">
                {t("recordShortcut.label")}
              </span>
              <span className="text-xs text-muted">{t("recordShortcut.description")}</span>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <ShortcutField />
              <ResetButton />
            </div>
          </div>
          {feedback && <Feedback feedback={feedback} />}
        </div>
        <ModePicker />
      </section>
      <DevIntent />
    </>
  );
}

/** The current shortcut as key caps; click to capture a new one. */
function ShortcutField() {
  const { t } = useTranslation();
  const [current] = useSetting("recordShortcut");
  const capture = useRecordShortcut((s) => s.capture);
  const beginCapture = useRecordShortcut((s) => s.beginCapture);
  const endCapture = useRecordShortcut((s) => s.endCapture);
  const label = useKeyLabel();
  const field = useRef<HTMLButtonElement>(null);
  const capturing = capture !== null;

  // Clicking anywhere outside the field, or the window losing focus, ends capture unchanged.
  useEffect(() => {
    if (!capturing) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!field.current?.contains(event.target as Node)) void endCapture();
    };
    const onBlur = () => void endCapture();
    document.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("blur", onBlur);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("blur", onBlur);
    };
  }, [capturing, endCapture]);

  const held = capture ? heldCombination(capture) : "";

  return (
    <div className="flex flex-col items-end gap-1">
      <button
        ref={field}
        type="button"
        aria-label={
          capturing ? undefined : t("recordShortcut.change", { shortcut: label(current) })
        }
        aria-describedby="record-shortcut-label"
        data-testid="record-shortcut-field"
        onClick={() => void beginCapture()}
        className={`flex min-h-10 min-w-48 items-center justify-center gap-1.5 rounded-lg border px-3 py-1.5 transition-colors duration-150 ${
          capturing
            ? "border-accent bg-accent-soft text-accent-soft-fg"
            : "border-line bg-raised/40 hover:border-accent/60 hover:bg-raised"
        }`}
      >
        {capturing ? (
          <span aria-live="polite" className="flex items-center gap-1.5">
            {held ? <KeyCaps combination={held} /> : t("recordShortcut.capturing")}
          </span>
        ) : (
          <KeyCaps combination={current} />
        )}
      </button>
      {capturing && <span className="text-xs text-muted">{t("recordShortcut.captureHint")}</span>}
    </div>
  );
}

function KeyCaps({ combination }: { combination: string }) {
  const label = useKeyLabel();
  return (
    <span className="flex items-center gap-1" data-testid="key-caps">
      {keysOf(combination).map((key, i) => (
        <span key={key} className="flex items-center gap-1">
          {i > 0 && (
            <span className="text-xs text-muted" aria-hidden="true">
              +
            </span>
          )}
          <kbd className="rounded-md border border-line bg-surface px-2 py-0.5 font-sans text-xs font-medium shadow-[0_1px_0_var(--color-line)]">
            {label(key)}
          </kbd>
        </span>
      ))}
    </span>
  );
}

/** Key names in the UI Language where they differ ("Spacja", "Prawy Alt"). */
function useKeyLabel() {
  const { t } = useTranslation();
  const names: Partial<Record<string, string>> = t("keys", { returnObjects: true });
  return (combination: string) =>
    keysOf(combination)
      .map((key) => names[key] ?? key)
      .join(" + ");
}

function ResetButton() {
  const { t } = useTranslation();
  const reset = useRecordShortcut((s) => s.reset);
  const [current] = useSetting("recordShortcut");
  const isDefault = current === DEFAULT_RECORD_SHORTCUT;
  return (
    <button
      type="button"
      onClick={() => void reset()}
      disabled={isDefault}
      className="rounded-lg px-3 py-1.5 text-sm text-accent-strong transition-colors duration-150 hover:bg-accent-soft disabled:cursor-default disabled:text-muted disabled:opacity-60 disabled:hover:bg-transparent"
    >
      {t("recordShortcut.reset")}
    </button>
  );
}

function Feedback({ feedback }: { feedback: ShortcutFeedback }) {
  const { t } = useTranslation();
  const label = useKeyLabel();
  let message: string;
  if (feedback.kind === "captureUnavailable") {
    message = t("recordShortcut.captureUnavailable");
  } else {
    const shortcut = label(feedback.proposal);
    const reason =
      feedback.error.kind === "notAllowed"
        ? t(`recordShortcut.problems.${feedback.error.problem}`, { shortcut })
        : t("recordShortcut.activationFailed", { shortcut, reason: feedback.error.reason });
    message = `${reason} ${t("recordShortcut.keepsPrevious")}`;
  }
  return (
    <p role="alert" className="text-right text-sm text-danger">
      {message}
    </p>
  );
}

function ModePicker() {
  const { t } = useTranslation();
  const [mode, setMode] = useSetting("shortcutMode");
  return (
    <fieldset className="flex flex-col gap-3 px-6 py-4">
      <legend className="float-left mb-3 font-medium">{t("recordShortcut.mode.label")}</legend>
      <div className="clear-left grid grid-cols-2 gap-3">
        {MODES.map((option) => {
          const checked = mode === option;
          return (
            <label
              key={option}
              className={`flex cursor-pointer gap-3 rounded-xl border px-4 py-3 transition-colors duration-150 has-focus-visible:outline-2 has-focus-visible:outline-focus ${
                checked ? "border-accent bg-accent-soft/60" : "border-line hover:bg-raised/50"
              }`}
            >
              <input
                type="radio"
                name="shortcut-mode"
                value={option}
                checked={checked}
                onChange={() => void setMode(option)}
                className="mt-1 accent-accent-strong"
              />
              <span className="flex flex-col gap-0.5">
                <span className="font-medium">{t(`recordShortcut.mode.${option}`)}</span>
                <span className="text-xs text-muted">{t(`recordShortcut.mode.${option}Hint`)}</span>
              </span>
            </label>
          );
        })}
      </div>
    </fieldset>
  );
}

/** TEMPORARY (remove with #13): shows the last Record Shortcut intent for checking the hook. */
function DevIntent() {
  const { t } = useTranslation();
  const intent = useRecordShortcut((s) => s.lastIntent);
  return (
    <section className="flex items-center justify-between gap-6 rounded-card border border-dashed border-line px-6 py-4">
      <div className="flex flex-col items-start gap-1">
        <span className="rounded-full bg-accent-soft px-2.5 py-0.5 text-xs font-medium text-accent-soft-fg">
          {t("devIntent.badge")}
        </span>
        <span className="text-xs text-muted">{t("devIntent.note")}</span>
      </div>
      <p className="text-sm">
        <span className="text-muted">{t("devIntent.label")}: </span>
        <span
          data-testid="dev-intent"
          className={`font-medium ${intent === "start" ? "text-accent-strong" : ""}`}
        >
          {t(intent ? `devIntent.${intent}` : "devIntent.none")}
        </span>
      </p>
    </section>
  );
}
