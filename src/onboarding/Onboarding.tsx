import { type ReactNode, useEffect, useId, useRef, useState } from "react";
import { Trans, useTranslation } from "react-i18next";

import { type DictationStatus, commands, events } from "../bindings";
import { Button } from "../components/Button";
import {
  ArrowIcon,
  CheckIcon,
  ChevronIcon,
  DownloadIcon,
  EchoMark,
  ExternalIcon,
  FolderIcon,
  InfoIcon,
  KeyboardIcon,
  LockIcon,
} from "../components/icons";
import { Keycap } from "../components/Keycap";
import { UiLanguagePicker } from "../components/UiLanguagePicker";
import { ModelChooser } from "../models/ModelChooser";
import { useDictation } from "../store/dictation";
import { useModels } from "../store/models";
import { useSetting } from "../store/settings";
import { useShell } from "../store/shell";
import {
  checkMicrophoneAccess,
  recommendedModel,
  useComputeHardware,
  useModelActive,
  useRecordShortcut,
} from "./sources";
import {
  type MicrophoneAccess,
  type OnboardingStep,
  currentStep,
  nextStep,
  previousStep,
  shownStep,
  visibleSteps,
} from "./steps";

/** How often the Microphone access step re-checks Windows privacy settings (rule 2.2). */
const MICROPHONE_RECHECK_MS = 2000;

/**
 * How long Try it shows "Echo heard you." before it opens the main window by itself (rule 2.4),
 * counted from the moment text first arrives; the step fades out over the last `LEAVE_FADE_MS`.
 */
const AUTO_ADVANCE_MS = 2500;
const LEAVE_FADE_MS = 400;

/**
 * First-run onboarding (settings-and-first-run.md rules 1–4), shown in the main window until the
 * user presses "Finish" or "Finish later". It always opens at the first incomplete step; Back
 * revisits earlier steps without undoing them (rule 2a).
 */
export function Onboarding() {
  const [welcomeDone, setWelcomeDone] = useSetting("onboardingWelcomeDone");
  const [, setCompleted] = useSetting("onboardingCompleted");
  const setPage = useShell((s) => s.setPage);
  const openSection = useShell((s) => s.openSection);
  const [microphoneSkipped, setMicrophoneSkipped] = useState(false);
  const [revisited, setRevisited] = useState<OnboardingStep | null>(null);
  const modelActive = useModelActive();
  const microphone = useMicrophoneAccess(welcomeDone && !microphoneSkipped);

  // Wait for the first privacy check rather than flash a step that is about to be replaced.
  const waiting = welcomeDone && !microphoneSkipped && microphone === null;
  const current = currentStep({ welcomeDone, microphone, microphoneSkipped, modelActive });
  const steps = visibleSteps(microphone === "denied");
  const step = shownStep(current, revisited, steps);
  const back = previousStep(step, steps);
  const stepSection = useFocusOnStepChange(waiting ? null : step);

  const goBack = () => {
    setRevisited(back);
  };
  const goForward = () => {
    setRevisited(nextStep(step, current, steps));
  };
  /** Leaves onboarding for a page of the main window (rules 2.4 and 3a). */
  const finishOn = (open: () => void) => {
    open();
    void setCompleted(true);
  };

  return (
    // Flat like the settings pages: no card around the step, the progress stepper on top.
    <div className="flex h-full flex-col items-center overflow-y-auto px-10 py-10 [scrollbar-gutter:stable]">
      <div className="flex w-full max-w-xl flex-col gap-8">
        <Progress current={step} steps={steps} />

        {!waiting && (
          <section ref={stepSection} className="flex flex-col gap-6">
            {step === "welcome" && (
              <WelcomeStep
                onContinue={() => {
                  if (welcomeDone) goForward();
                  else void setWelcomeDone(true);
                }}
              />
            )}
            {step === "microphone" && (
              <MicrophoneStep
                onBack={goBack}
                onSkip={() => {
                  setMicrophoneSkipped(true);
                  goForward();
                }}
              />
            )}
            {step === "model" && (
              <ModelStep
                onBack={goBack}
                onContinue={modelActive ? goForward : null}
                onFinishLater={() => {
                  finishOn(() => {
                    setPage("model");
                  });
                }}
              />
            )}
            {step === "tryIt" && (
              <TryItStep
                onBack={goBack}
                onFinish={() => void setCompleted(true)}
                onMicrophoneSettings={() => {
                  finishOn(() => {
                    openSection("dictation", "microphone");
                  });
                }}
                onModelSettings={() => {
                  finishOn(() => {
                    setPage("model");
                  });
                }}
              />
            )}
          </section>
        )}
      </div>
    </div>
  );
}

/**
 * Moves keyboard focus to the step's heading whenever the step changes, so screen readers
 * announce the new step (rule 2b). The first step shown is left alone. A step with a
 * `data-step-focus` element (Try it's test field, where a Dictation has to land) focuses that
 * instead, also when it is the first step shown; the field's description names the step. `null`
 * while no step shows.
 */
function useFocusOnStepChange(step: OnboardingStep | null) {
  const section = useRef<HTMLElement>(null);
  const previous = useRef<OnboardingStep | null>(null);
  useEffect(() => {
    if (step === null) return;
    const changed = previous.current !== null && previous.current !== step;
    const target = section.current?.querySelector<HTMLElement>("[data-step-focus]");
    if (target) target.focus();
    else if (changed) section.current?.querySelector("h1")?.focus();
    previous.current = step;
  }, [step]);
  return section;
}

/**
 * The latest Windows microphone privacy check; `null` until the first answer. While `watch` is
 * on and access is denied, it re-checks every 2 s and whenever the window regains focus.
 */
function useMicrophoneAccess(watch: boolean): MicrophoneAccess | null {
  const [access, setAccess] = useState<MicrophoneAccess | null>(null);
  const polling = watch && access !== "allowed";

  useEffect(() => {
    let cancelled = false;
    const check = () => {
      void checkMicrophoneAccess().then((answer) => {
        if (!cancelled) setAccess(answer);
      });
    };
    check();
    if (!polling) {
      return () => {
        cancelled = true;
      };
    }
    const timer = setInterval(check, MICROPHONE_RECHECK_MS);
    window.addEventListener("focus", check);
    return () => {
      cancelled = true;
      clearInterval(timer);
      window.removeEventListener("focus", check);
    };
  }, [polling]);

  return access;
}

/**
 * The steps in a row joined by hairlines: a check mark on completed steps and a filled lavender
 * dot on the current one, so the state never rests on colour alone (rule 2b).
 */
function Progress({ current, steps }: { current: OnboardingStep; steps: OnboardingStep[] }) {
  const { t } = useTranslation();
  const currentIndex = steps.indexOf(current);
  return (
    <ol aria-label={t("onboarding.progress")} className="flex items-center gap-3 text-note">
      {steps.map((step, index) => {
        const state = index < currentIndex ? "done" : index === currentIndex ? "current" : "next";
        return (
          <li
            key={step}
            aria-current={state === "current" ? "step" : undefined}
            className="flex flex-1 items-center gap-3 last:flex-none"
          >
            <span
              className={`flex shrink-0 items-center gap-1.5 whitespace-nowrap ${
                state === "current"
                  ? "font-medium text-fg"
                  : state === "done"
                    ? "text-fg"
                    : "text-muted"
              }`}
            >
              {state === "done" ? (
                <CheckIcon className="shrink-0" />
              ) : (
                <span
                  aria-hidden="true"
                  className={`size-2 shrink-0 rounded-full ${
                    state === "current" ? "bg-accent" : "border border-muted/70"
                  }`}
                />
              )}
              {t(`onboarding.steps.${step}`)}
              {state === "done" && <span className="sr-only">, {t("onboarding.stepDone")}</span>}
            </span>
            {index < steps.length - 1 && (
              <span aria-hidden="true" className="h-px min-w-4 flex-1 bg-line" />
            )}
          </li>
        );
      })}
    </ol>
  );
}

/** A step's title and lead, set like a settings page's header. */
function StepHeader({ title, lead, titleId }: { title: string; lead: string; titleId?: string }) {
  return (
    <header className="flex flex-col gap-1">
      <h1
        id={titleId}
        tabIndex={-1}
        className="font-display text-xl font-semibold tracking-[-0.01em] text-balance outline-none"
      >
        {title}
      </h1>
      <p className="text-pretty text-muted">{lead}</p>
    </header>
  );
}

function PrimaryButton({ children, onClick }: { children: ReactNode; onClick: () => void }) {
  return (
    <Button size="default" onClick={onClick}>
      {children}
    </Button>
  );
}

/** The navigation row under a step: Back on the left, the way forward on the right. */
function StepFooter({ children }: { children: ReactNode }) {
  return <footer className="mt-2 flex items-center justify-between gap-6">{children}</footer>;
}

/** A borderless text action; `edge` lines its text up with the column's left or right edge. */
function QuietButton({
  children,
  onClick,
  edge,
}: {
  children: ReactNode;
  onClick: () => void;
  edge?: "left" | "right";
}) {
  const align = edge === "left" ? "-ml-3.5" : edge === "right" ? "-mr-3.5" : "";
  return (
    <Button
      variant="quiet"
      onClick={onClick}
      className={`inline-flex items-center gap-1.5 ${align}`}
    >
      {children}
    </Button>
  );
}

/** A forward action that leaves something undone, with what that means under it. */
function DeferAction({
  label,
  note,
  onClick,
}: {
  label: string;
  note: string;
  onClick: () => void;
}) {
  return (
    // The note hangs under the button, so the button lines up with "Back" on the left.
    <div className="relative min-w-0">
      <QuietButton onClick={onClick} edge="right">
        <span className="text-fg">{label}</span>
      </QuietButton>
      <span
        title={note}
        className="absolute top-full right-0 w-max max-w-[22rem] truncate text-note text-muted"
      >
        {note}
      </span>
    </div>
  );
}

/** "Back" on every step after the first (rule 2a). */
function BackButton({ onClick }: { onClick: () => void }) {
  const { t } = useTranslation();
  return (
    <QuietButton onClick={onClick} edge="left">
      {t("onboarding.back")}
    </QuietButton>
  );
}

function WelcomeStep({ onContinue }: { onContinue: () => void }) {
  const { t } = useTranslation();
  const { mode } = useRecordShortcut();
  const points = [
    { key: "local", Icon: LockIcon },
    { key: "model", Icon: DownloadIcon },
    { key: mode === "pushToTalk" ? "shortcutHold" : "shortcutPress", Icon: KeyboardIcon },
  ] as const;
  return (
    <>
      {/* The one centred step: a greeting with the logo, before the steps that do something. */}
      <header className="flex flex-col items-center gap-2.5 pt-2 text-center">
        <EchoMark className="mb-1 size-10" />
        <h1 tabIndex={-1} className="font-display text-title text-balance outline-none">
          {t("onboarding.welcome.title")}
        </h1>
        <p className="text-pretty text-muted">{t("onboarding.welcome.lead")}</p>
      </header>
      <ul className="flex flex-col gap-3 px-2">
        {points.map(({ key, Icon }) => (
          <li key={key} className="flex gap-3">
            <Icon className="mt-0.5 shrink-0 text-muted" />
            <span className="text-pretty">{t(`onboarding.welcome.${key}`)}</span>
          </li>
        ))}
      </ul>
      <StepFooter>
        <UiLanguagePicker />
        <PrimaryButton onClick={onContinue}>{t("onboarding.welcome.start")}</PrimaryButton>
      </StepFooter>
    </>
  );
}

function MicrophoneStep({ onBack, onSkip }: { onBack: () => void; onSkip: () => void }) {
  const { t } = useTranslation();
  return (
    <>
      <StepHeader title={t("onboarding.microphone.title")} lead={t("onboarding.microphone.lead")} />
      <p className="text-pretty">{t("onboarding.microphone.howTo")}</p>
      <div className="flex items-center justify-between gap-6">
        <p aria-live="polite" className="flex min-w-0 items-center gap-2 text-note text-muted">
          <span
            aria-hidden="true"
            className="size-2 shrink-0 animate-pulse rounded-full bg-accent"
          />
          <span title={t("onboarding.microphone.checking")} className="truncate">
            {t("onboarding.microphone.checking")}
          </span>
        </p>
        <PrimaryButton onClick={() => void commands.openMicrophonePrivacySettings()}>
          {t("onboarding.microphone.open")}
        </PrimaryButton>
      </div>
      <StepFooter>
        <BackButton onClick={onBack} />
        <DeferAction
          label={t("onboarding.microphone.skip")}
          note={t("onboarding.microphone.skipNote")}
          onClick={onSkip}
        />
      </StepFooter>
    </>
  );
}

/**
 * Choose a Model (rule 2.3): what Echo found about the graphics card, the three Models each with
 * its own Download, the recommended one marked, Back, and "Finish later" — or "Continue" when
 * revisited with a Model active.
 */
function ModelStep({
  onBack,
  onContinue,
  onFinishLater,
}: {
  onBack: () => void;
  onContinue: (() => void) | null;
  onFinishLater: () => void;
}) {
  const { t } = useTranslation();
  const hardware = useComputeHardware();
  const models = useModels((s) => s.state);
  const recommended = hardware === null ? null : recommendedModel(hardware);
  const recommendedName = models?.models.find((m) => m.id === recommended)?.name;
  const found =
    recommendedName && hardware !== "unknown"
      ? t(hardware === "gpu" ? "onboarding.model.foundGpu" : "onboarding.model.noGpu", {
          model: recommendedName,
        })
      : null;
  return (
    <>
      <StepHeader title={t("onboarding.model.title")} lead={t("onboarding.model.lead")} />
      <div className="-mt-1 flex flex-col gap-1">
        {found && (
          <p className="flex items-center gap-2 text-muted">
            <InfoIcon className="shrink-0" />
            <span title={found} className="truncate">
              {found}
            </span>
          </p>
        )}
        {recommended !== null && <ModelChooser recommended={recommended} />}
      </div>
      <StepFooter>
        <BackButton onClick={onBack} />
        {onContinue ? (
          <PrimaryButton onClick={onContinue}>{t("onboarding.model.continue")}</PrimaryButton>
        ) : (
          <DeferAction
            label={t("onboarding.model.later")}
            note={t("onboarding.model.laterNote")}
            onClick={onFinishLater}
          />
        )}
      </StepFooter>
    </>
  );
}

/**
 * Try it (rule 2.4): the Record Shortcut, the sample phrase as a large test field that follows
 * the Dictation's phases and acknowledges arriving text, "Nothing appeared?" with the likely
 * causes and their fixes, and a move to the main window 2.5 s after the words arrive unless the
 * user starts another Dictation or works in the field first.
 */
function TryItStep({
  onBack,
  onFinish,
  onMicrophoneSettings,
  onModelSettings,
}: {
  onBack: () => void;
  onFinish: () => void;
  onMicrophoneSettings: () => void;
  onModelSettings: () => void;
}) {
  const { t } = useTranslation();
  const shortcut = useRecordShortcut();
  const status = useDictation((s) => s.status);
  const loadDictation = useDictation((s) => s.load);
  const [text, setText] = useState("");
  const [advanceCancelled, setAdvanceCancelled] = useState(false);
  const [leaving, setLeaving] = useState(false);
  const [seenState, setSeenState] = useState(status.state);
  const titleId = useId();
  const instructionId = useId();

  const heard = text.trim() !== "";
  const phase = tryItPhase(status, heard);
  const advancing = heard && !advanceCancelled;

  // A new Dictation after the words arrived means the user wants another go: stay here.
  if (status.state !== seenState) {
    setSeenState(status.state);
    if (status.state === "recording" && heard) {
      setAdvanceCancelled(true);
      setLeaving(false);
    }
  }

  useEffect(() => {
    void loadDictation();
  }, [loadDictation]);

  // During onboarding the backend sends each Transcript here instead of inserting it into the
  // focused application, so it lands in the field wherever focus is (rule 2.4).
  useEffect(() => {
    const stop = events.tryItTranscript.listen((event) => {
      setText(event.payload);
    });
    return () => {
      void stop.then((unlisten) => {
        unlisten();
      });
    };
  }, []);

  // The parent passes a new callback on every render; the timer must not restart for that.
  const finishRef = useRef(onFinish);
  useEffect(() => {
    finishRef.current = onFinish;
  }, [onFinish]);

  useEffect(() => {
    if (!advancing) return;
    const fade = setTimeout(() => {
      setLeaving(true);
    }, AUTO_ADVANCE_MS - LEAVE_FADE_MS);
    const finish = setTimeout(() => {
      finishRef.current();
    }, AUTO_ADVANCE_MS);
    return () => {
      clearTimeout(fade);
      clearTimeout(finish);
    };
  }, [advancing]);

  /** The user works in the field after the words arrived: they finish with "Finish" instead. */
  const cancelAdvance = () => {
    if (!heard) return;
    setAdvanceCancelled(true);
    setLeaving(false);
  };

  return (
    <div
      className={`flex flex-col gap-6 transition-opacity duration-400 ease-out-strong ${
        leaving ? "opacity-0" : ""
      }`}
    >
      <StepHeader
        titleId={titleId}
        title={t("onboarding.tryIt.title")}
        lead={t("onboarding.tryIt.lead")}
      />
      <p id={instructionId}>
        <Trans
          i18nKey={
            shortcut.mode === "pushToTalk"
              ? "onboarding.tryIt.holdShortcut"
              : "onboarding.tryIt.pressShortcut"
          }
          values={{ shortcut: shortcut.label }}
          components={{
            kbd: <Keycap />,
          }}
        />
      </p>
      {/* The hero: one large borderless field with the sample phrase drawn behind it, so the
          phrase can follow the Dictation and the dictated words land in its place. */}
      <div className="relative -mt-2 font-display text-[1.75rem] leading-tight font-semibold tracking-[-0.015em]">
        <p
          aria-hidden="true"
          data-phase={heard ? "heard" : phase}
          className="try-it-phrase pointer-events-none absolute inset-x-0 top-0 text-muted"
        >
          {t("onboarding.tryIt.sample")}
        </p>
        <textarea
          data-step-focus
          rows={2}
          value={text}
          aria-label={t("onboarding.tryIt.fieldLabel")}
          aria-describedby={`${titleId} ${instructionId}`}
          placeholder={t("onboarding.tryIt.sample")}
          onChange={(event) => {
            setText(event.target.value);
          }}
          onPointerDown={cancelAdvance}
          onFocus={(event) => {
            // Coming back to the window refocuses the field without a related target.
            if (event.relatedTarget !== null) cancelAdvance();
          }}
          onKeyDown={() => {
            // Keys during an Insertion are Echo's own typing, not the user's.
            if (status.state === "idle") cancelAdvance();
          }}
          className={`relative block w-full resize-none border-0 bg-transparent p-0 text-fg outline-none select-text placeholder:text-transparent ${
            heard ? "try-it-words" : ""
          } ${phase === "idle" || heard ? "caret-accent" : "caret-transparent"}`}
        />
      </div>
      <TryItStatus phase={phase} />
      <TroubleHints onMicrophoneSettings={onMicrophoneSettings} onModelSettings={onModelSettings} />
      <StepFooter>
        <BackButton onClick={onBack} />
        <div className="flex min-w-0 items-center gap-6">
          {advancing ? (
            <span key="opening" className="try-it-status truncate text-note text-muted">
              {t("onboarding.tryIt.opening")}
            </span>
          ) : (
            <span title={t("onboarding.tryIt.later")} className="truncate text-note text-muted">
              {t("onboarding.tryIt.later")}
            </span>
          )}
          <PrimaryButton onClick={onFinish}>{t("onboarding.tryIt.finish")}</PrimaryButton>
        </div>
      </StepFooter>
    </div>
  );
}

/** What Try it shows of the Dictation (rule 2.4); `heard` once any text is in the test field. */
type TryItPhase = "idle" | "gettingReady" | "listening" | "transcribing" | "heard";

function tryItPhase(status: DictationStatus, heard: boolean): TryItPhase {
  if (status.state === "recording") return status.listening ? "listening" : "gettingReady";
  if (status.state === "transcribing") return "transcribing";
  if (heard) return "heard";
  return status.state === "inserting" ? "transcribing" : "idle";
}

/**
 * The fixed-height line under the test field: the Dictation's phase in the Overlay's words, then
 * a lavender check mark that draws itself once text arrives. Announced as a status.
 */
function TryItStatus({ phase }: { phase: TryItPhase }) {
  const { t } = useTranslation();
  return (
    <p role="status" className="-mt-3 flex h-5 items-center text-sm text-muted">
      {phase !== "idle" && (
        <span key={phase} className="try-it-status flex items-center gap-2">
          {phase === "heard" ? (
            <>
              <svg
                viewBox="0 0 24 24"
                width="15"
                height="15"
                fill="none"
                stroke="currentColor"
                strokeWidth="2.4"
                strokeLinecap="round"
                strokeLinejoin="round"
                aria-hidden="true"
                className="try-it-check shrink-0 text-accent"
              >
                <path pathLength={1} d="m5 12.5 4.5 4.5L19 7.5" />
              </svg>
              <span className="text-fg">{t("onboarding.tryIt.worked")}</span>
            </>
          ) : (
            <>
              <span
                aria-hidden="true"
                className={`try-it-pulse size-2 shrink-0 rounded-full ${
                  phase === "gettingReady" ? "bg-muted" : "bg-accent"
                }`}
              />
              {t(
                phase === "gettingReady"
                  ? "overlay.gettingReady"
                  : phase === "listening"
                    ? "overlay.listening"
                    : "overlay.transcribing",
              )}
            </>
          )}
        </span>
      )}
    </p>
  );
}

/** "Nothing appeared?": the likely causes of a silent first Dictation, each with its fix. */
function TroubleHints({
  onMicrophoneSettings,
  onModelSettings,
}: {
  onMicrophoneSettings: () => void;
  onModelSettings: () => void;
}) {
  const { t } = useTranslation();
  return (
    <details className="group">
      <summary className="flex w-fit cursor-pointer list-none items-center gap-1.5 rounded-md text-muted transition-colors duration-150 hover:text-fg focus-visible:outline-2 focus-visible:outline-focus [&::-webkit-details-marker]:hidden">
        <span className="-rotate-90 transition-transform duration-150 group-open:rotate-0">
          <ChevronIcon />
        </span>
        {t("onboarding.tryIt.trouble.summary")}
      </summary>
      <ul className="mt-1">
        <TroubleHint text={t("onboarding.tryIt.trouble.microphone")}>
          <QuietButton onClick={onMicrophoneSettings}>
            <ArrowIcon />
            {t("onboarding.tryIt.trouble.microphoneSettings")}
          </QuietButton>
          <QuietButton onClick={() => void commands.openMicrophonePrivacySettings()}>
            <ExternalIcon />
            {t("onboarding.tryIt.trouble.privacySettings")}
          </QuietButton>
        </TroubleHint>
        <TroubleHint text={t("onboarding.tryIt.trouble.model")}>
          <QuietButton onClick={onModelSettings}>
            <ArrowIcon />
            {t("onboarding.tryIt.trouble.modelSettings")}
          </QuietButton>
        </TroubleHint>
        <TroubleHint text={t("onboarding.tryIt.trouble.other")}>
          <QuietButton onClick={() => void commands.openLogFolder()}>
            <FolderIcon />
            {t("settings.logFolder.open")}
          </QuietButton>
        </TroubleHint>
      </ul>
    </details>
  );
}

/** One likely cause as a flat row: what may be wrong, then the quiet actions that fix it. */
function TroubleHint({ text, children }: { text: string; children: ReactNode }) {
  return (
    <li className="flex flex-col gap-1 border-b border-line py-3 last:border-b-0">
      <span className="text-pretty">{text}</span>
      <span className="-ml-3.5 flex flex-wrap gap-x-1">{children}</span>
    </li>
  );
}
