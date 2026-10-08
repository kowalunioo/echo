import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import { type ShortcutMode, events } from "../bindings";
import { heldCombination, keysOf } from "../shortcut/capture";
import { useKeyLabel } from "../shortcut/keyLabel";
import {
  DEFAULT_CANCEL_SHORTCUT,
  DEFAULT_RECORD_SHORTCUT,
  SETTING_OF,
  type ShortcutFeedback,
  type ShortcutTarget,
  useRecordShortcut,
} from "../store/recordShortcut";
import { useSetting } from "../store/settings";
import { FailureMessage } from "./FailureMessage";
import { PencilIcon, ResetIcon } from "./icons";
import { Keycap } from "./Keycap";
import { SectionHeading } from "./SectionHeading";
import { SettingRow } from "./SettingRow";

const MODES: readonly ShortcutMode[] = ["pushToTalk", "toggle"];

/**
 * Record Shortcut and its mode, and the Cancel Shortcut, on the Dictation page
 * (record-shortcut.md and cancel-shortcut.md, "UI").
 */
export function RecordShortcutSettings() {
  const captureKey = useRecordShortcut((s) => s.captureKey);
  const feedback = useRecordShortcut((s) => s.feedback);

  useEffect(() => {
    const subscriptions = [
      events.capturedKeyEvent.listen((e) => void captureKey(e.payload.key, e.payload.pressed)),
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
  }, [captureKey]);

  const { t } = useTranslation();
  return (
    <section aria-labelledby="shortcuts-heading">
      <SectionHeading id="shortcuts-heading">
        {t("pages.dictation.sections.shortcuts")}
      </SectionHeading>
      <ShortcutRow target="record" feedback={feedback?.target === "record" ? feedback : null} />
      <ModePicker />
      <ShortcutRow target="cancel" feedback={feedback?.target === "cancel" ? feedback : null} />
    </section>
  );
}

/** The i18n section of each shortcut's own texts. */
const TEXTS = { record: "recordShortcut", cancel: "cancelShortcut" } as const;

/** One shortcut: name, description, capture field and reset button (cancel-shortcut.md "UI"). */
function ShortcutRow({
  target,
  feedback,
}: {
  target: ShortcutTarget;
  feedback: ShortcutFeedback | null;
}) {
  const { t } = useTranslation();
  const capturing = useRecordShortcut((s) => s.target === target && s.capture !== null);
  return (
    <SettingRow
      label={t(`${TEXTS[target]}.label`)}
      description={t(`${TEXTS[target]}.${capturing ? "captureHint" : "description"}`)}
      labelId={`${target}-shortcut-label`}
      below={feedback && <Feedback feedback={feedback} />}
    >
      <ShortcutField target={target} />
      <ResetButton target={target} />
    </SettingRow>
  );
}

/**
 * The current shortcut as key caps; on hover a light grey fill and a pencil mark it as editable,
 * and a click captures a new one. While capturing it shows the held keys, and a Cancel button sits
 * next to it.
 */
function ShortcutField({ target }: { target: ShortcutTarget }) {
  const { t } = useTranslation();
  const [current] = useSetting(SETTING_OF[target]);
  const capture = useRecordShortcut((s) => (s.target === target ? s.capture : null));
  const beginCapture = useRecordShortcut((s) => s.beginCapture);
  const endCapture = useRecordShortcut((s) => s.endCapture);
  const label = useKeyLabel();
  const field = useRef<HTMLDivElement>(null);
  const capturing = capture !== null;

  // Clicking anywhere outside the field (and its Cancel button), or the window losing focus, ends
  // capture unchanged.
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
    <div ref={field} className="flex items-center gap-3">
      <button
        type="button"
        aria-label={
          capturing ? undefined : t(`${TEXTS[target]}.change`, { shortcut: label(current) })
        }
        aria-describedby={`${target}-shortcut-label`}
        data-testid={`${target}-shortcut-field`}
        onClick={() => {
          if (!capturing) void beginCapture(target);
        }}
        title={capturing ? undefined : t(`${TEXTS[target]}.change`, { shortcut: label(current) })}
        className={`group -mr-2 flex min-h-8 items-center gap-2 rounded-lg px-2 transition-colors duration-150 ${
          capturing ? "" : "cursor-pointer hover:bg-raised/50"
        }`}
      >
        {capturing ? (
          <span
            aria-live="polite"
            className="flex items-center gap-2 text-sm font-medium text-accent-strong"
          >
            <span
              aria-hidden="true"
              className="size-1.5 rounded-full bg-accent shadow-[0_0_0_3px_var(--color-accent-soft)]"
            />
            {held ? <KeyCaps combination={held} /> : t("recordShortcut.capturing")}
          </span>
        ) : (
          <>
            {/* Always takes its space, so the hover fill does not shift the row. */}
            <span className="text-muted opacity-0 transition-opacity duration-150 group-hover:opacity-100 group-focus-visible:opacity-100">
              <PencilIcon />
            </span>
            <KeyCaps combination={current} />
          </>
        )}
      </button>
      {capturing && (
        <button
          type="button"
          onClick={() => void endCapture()}
          className="hit-target flex items-center gap-1.5 rounded-md text-sm text-muted underline-offset-4 hover:text-fg hover:underline"
        >
          {/* Esc ends capture of the Record Shortcut; for the Cancel Shortcut it is a value. */}
          {target === "record" && (
            <span aria-hidden="true">
              <Keycap size="small">{label("Escape")}</Keycap>
            </span>
          )}
          {t("recordShortcut.stopCapture")}
        </button>
      )}
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
            <span className="text-note text-muted" aria-hidden="true">
              +
            </span>
          )}
          <Keycap size="small">{label(key)}</Keycap>
        </span>
      ))}
    </span>
  );
}

const DEFAULT_OF = { record: DEFAULT_RECORD_SHORTCUT, cancel: DEFAULT_CANCEL_SHORTCUT } as const;

function ResetButton({ target }: { target: ShortcutTarget }) {
  const { t } = useTranslation();
  const reset = useRecordShortcut((s) => s.reset);
  const [current] = useSetting(SETTING_OF[target]);
  // Only shown once the shortcut differs from its default, at the row's right edge.
  if (current === DEFAULT_OF[target]) return null;
  return (
    <button
      type="button"
      aria-label={t("recordShortcut.reset")}
      title={t("recordShortcut.reset")}
      aria-describedby={`${target}-shortcut-label`}
      onClick={() => void reset(target)}
      className="-mr-1.5 grid size-7 shrink-0 place-items-center rounded-md text-muted transition-colors duration-150 hover:bg-raised hover:text-fg"
    >
      <ResetIcon />
    </button>
  );
}

function Feedback({ feedback }: { feedback: ShortcutFeedback }) {
  const { t } = useTranslation();
  const label = useKeyLabel();
  let message: string;
  let detail: string | undefined;
  if (feedback.kind === "captureUnavailable") {
    message = t("recordShortcut.captureUnavailable");
  } else {
    const shortcut = label(feedback.proposal);
    let reason: string;
    if (feedback.error.kind === "notAllowed") {
      reason = t(`recordShortcut.problems.${feedback.error.problem}`, { shortcut });
    } else {
      reason = t("recordShortcut.activationFailed", { shortcut });
      detail = t("recordShortcut.failureDetail", { detail: feedback.error.reason });
    }
    message = `${reason} ${t("recordShortcut.keepsPrevious")}`;
  }
  return (
    <FailureMessage
      message={message}
      detail={detail}
      className="items-end text-right text-sm text-danger"
    />
  );
}

function ModePicker() {
  const { t } = useTranslation();
  const [mode, setMode] = useSetting("shortcutMode");
  // A segmented control built from real radios; the chosen mode's hint is the row's description.
  return (
    <SettingRow
      label={t("recordShortcut.mode.label")}
      description={t(`recordShortcut.mode.${mode}Hint`)}
      labelId="shortcut-mode-label"
    >
      <div
        role="radiogroup"
        aria-labelledby="shortcut-mode-label"
        className="flex rounded-lg border border-line p-0.5"
      >
        {MODES.map((option) => {
          const checked = mode === option;
          return (
            <label
              key={option}
              title={t(`recordShortcut.mode.${option}`)}
              className={`relative max-w-52 cursor-pointer truncate rounded-md px-3 py-1 text-sm transition-colors duration-150 has-focus-visible:outline-2 has-focus-visible:outline-focus ${
                checked ? "bg-raised font-medium text-fg" : "text-muted hover:text-fg"
              }`}
            >
              <input
                type="radio"
                name="shortcut-mode"
                value={option}
                checked={checked}
                onChange={() => void setMode(option)}
                aria-labelledby={`shortcut-mode-${option}`}
                aria-describedby={`shortcut-mode-${option}-hint`}
                className="sr-only"
              />
              <span id={`shortcut-mode-${option}`}>{t(`recordShortcut.mode.${option}`)}</span>
              <span id={`shortcut-mode-${option}-hint`} className="sr-only">
                {t(`recordShortcut.mode.${option}Hint`)}
              </span>
            </label>
          );
        })}
      </div>
    </SettingRow>
  );
}
