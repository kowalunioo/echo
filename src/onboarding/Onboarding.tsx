import { type ReactNode, useEffect, useRef, useState } from "react";
import { Trans, useTranslation } from "react-i18next";

import { commands } from "../bindings";
import { Button } from "../components/Button";
import { CheckIcon, EchoMark } from "../components/icons";
import { Keycap } from "../components/Keycap";
import { UiLanguagePicker } from "../components/UiLanguagePicker";
import { ModelChooser } from "../models/ModelChooser";
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
 * First-run onboarding (settings-and-first-run.md rules 1–4), shown in the main window until the
 * user presses "Finish" or "Finish later". It always opens at the first incomplete step; Back
 * revisits earlier steps without undoing them (rule 2a).
 */
export function Onboarding() {
  const { t } = useTranslation();
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
    <div className="flex h-full flex-col items-center overflow-y-auto px-6 py-10">
      <div className="flex w-full max-w-xl flex-col gap-8">
        <header className="flex items-center justify-between gap-6">
          <div className="flex items-center gap-3">
            <EchoMark className="size-8" />
            <span className="font-display text-base font-semibold">{t("app.name")}</span>
          </div>
          <Progress current={step} steps={steps} />
        </header>

        {!waiting && (
          <section
            ref={stepSection}
            className="flex flex-col gap-6 rounded-card border border-line bg-surface p-8"
          >
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
 * announce the new step (rule 2b). The first step shown is left alone. `null` while no step
 * shows.
 */
function useFocusOnStepChange(step: OnboardingStep | null) {
  const section = useRef<HTMLElement>(null);
  const previous = useRef<OnboardingStep | null>(null);
  useEffect(() => {
    if (step === null) return;
    if (previous.current !== null && previous.current !== step) {
      section.current?.querySelector("h1")?.focus();
    }
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

function Progress({ current, steps }: { current: OnboardingStep; steps: OnboardingStep[] }) {
  const { t } = useTranslation();
  const currentIndex = steps.indexOf(current);
  return (
    <ol aria-label={t("onboarding.progress")} className="flex items-center gap-2 text-xs">
      {steps.map((step, index) => {
        const state = index < currentIndex ? "done" : index === currentIndex ? "current" : "next";
        return (
          <li
            key={step}
            aria-current={state === "current" ? "step" : undefined}
            className={`flex items-center gap-1.5 rounded-full px-2.5 py-1 ${
              state === "current"
                ? "bg-accent-soft font-medium text-accent-soft-fg"
                : state === "done"
                  ? "text-fg"
                  : "text-muted"
            }`}
          >
            {state === "done" ? (
              <CheckIcon className="shrink-0 text-accent" />
            ) : (
              <span
                aria-hidden="true"
                className={`size-2 shrink-0 rounded-full ${
                  state === "current" ? "bg-accent" : "border border-muted/60"
                }`}
              />
            )}
            {t(`onboarding.steps.${step}`)}
            {state === "done" && <span className="sr-only">, {t("onboarding.stepDone")}</span>}
          </li>
        );
      })}
    </ol>
  );
}

function StepHeader({ title, lead }: { title: string; lead: string }) {
  return (
    <header className="flex flex-col gap-2">
      <h1 tabIndex={-1} className="font-display text-title text-balance outline-none">
        {title}
      </h1>
      <p className="text-muted">{lead}</p>
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

function TextButton({ children, onClick }: { children: ReactNode; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="hit-target rounded-md text-sm text-muted underline-offset-4 transition-colors duration-150 hover:text-fg hover:underline"
    >
      {children}
    </button>
  );
}

/** "Back" on every step after the first (rule 2a). */
function BackButton({ onClick }: { onClick: () => void }) {
  const { t } = useTranslation();
  return <TextButton onClick={onClick}>{t("onboarding.back")}</TextButton>;
}

function WelcomeStep({ onContinue }: { onContinue: () => void }) {
  const { t } = useTranslation();
  const { mode } = useRecordShortcut();
  const shortcutLine = mode === "pushToTalk" ? "shortcutHold" : "shortcutPress";
  return (
    <>
      <StepHeader title={t("onboarding.welcome.title")} lead={t("onboarding.welcome.lead")} />
      <ul className="flex flex-col gap-3">
        {(["local", "model", shortcutLine] as const).map((point) => (
          <li key={point} className="flex gap-3">
            <span aria-hidden="true" className="mt-2 size-1.5 shrink-0 rounded-full bg-accent" />
            <span>{t(`onboarding.welcome.${point}`)}</span>
          </li>
        ))}
      </ul>
      <div className="flex items-center justify-between gap-6 rounded-xl bg-raised/50 px-4 py-3">
        <span className="font-medium">{t("settings.uiLanguage.label")}</span>
        <UiLanguagePicker />
      </div>
      <footer className="flex justify-end">
        <PrimaryButton onClick={onContinue}>{t("onboarding.welcome.start")}</PrimaryButton>
      </footer>
    </>
  );
}

function MicrophoneStep({ onBack, onSkip }: { onBack: () => void; onSkip: () => void }) {
  const { t } = useTranslation();
  return (
    <>
      <StepHeader title={t("onboarding.microphone.title")} lead={t("onboarding.microphone.lead")} />
      <p>{t("onboarding.microphone.howTo")}</p>
      <p aria-live="polite" className="flex items-center gap-2 text-note text-muted">
        <span aria-hidden="true" className="size-2 animate-pulse rounded-full bg-accent" />
        {t("onboarding.microphone.checking")}
      </p>
      <footer className="flex items-center justify-between gap-6">
        <BackButton onClick={onBack} />
        <div className="flex items-center gap-6">
          <div className="flex flex-col items-end gap-0.5">
            <TextButton onClick={onSkip}>{t("onboarding.microphone.skip")}</TextButton>
            <span className="text-note text-muted">{t("onboarding.microphone.skipNote")}</span>
          </div>
          <PrimaryButton onClick={() => void commands.openMicrophonePrivacySettings()}>
            {t("onboarding.microphone.open")}
          </PrimaryButton>
        </div>
      </footer>
    </>
  );
}

/**
 * Choose a Model (rule 2.3): what Echo found about the graphics card, the recommended Model
 * pre-selected, Back, and "Finish later" — or "Continue" when revisited with a Model active.
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
  return (
    <>
      <StepHeader title={t("onboarding.model.title")} lead={t("onboarding.model.lead")} />
      {recommendedName && hardware !== "unknown" && (
        <p className="text-sm">
          {t(hardware === "gpu" ? "onboarding.model.foundGpu" : "onboarding.model.noGpu", {
            model: recommendedName,
          })}
        </p>
      )}
      {recommended !== null && <ModelChooser recommended={recommended} />}
      <footer className="flex items-center justify-between gap-6 border-t border-line pt-5">
        <BackButton onClick={onBack} />
        {onContinue ? (
          <PrimaryButton onClick={onContinue}>{t("onboarding.model.continue")}</PrimaryButton>
        ) : (
          <div className="flex flex-col items-end gap-0.5">
            <TextButton onClick={onFinishLater}>{t("onboarding.model.later")}</TextButton>
            <span className="text-note text-muted">{t("onboarding.model.laterNote")}</span>
          </div>
        )}
      </footer>
    </>
  );
}

/**
 * Try it (rule 2.4): the Record Shortcut, a test field that acknowledges arriving text, and
 * "Nothing appeared?" with the likely causes and their fixes.
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
  const [text, setText] = useState("");
  return (
    <>
      <StepHeader title={t("onboarding.tryIt.title")} lead={t("onboarding.tryIt.lead")} />
      <p className="text-base">
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
      <label className="flex flex-col gap-2">
        <span className="text-sm font-medium">{t("onboarding.tryIt.fieldLabel")}</span>
        <textarea
          rows={4}
          value={text}
          onChange={(event) => {
            setText(event.target.value);
          }}
          placeholder={t("onboarding.tryIt.fieldPlaceholder")}
          className="resize-none rounded-xl border border-control bg-bg px-4 py-3 select-text placeholder:text-muted focus-visible:outline-2 focus-visible:outline-focus"
        />
      </label>
      <p role="status" className="-mt-3 flex min-h-5 items-center gap-1.5 text-sm text-fg">
        {text.trim() !== "" && (
          <>
            <CheckIcon className="shrink-0 text-accent" />
            {t("onboarding.tryIt.worked")}
          </>
        )}
      </p>
      <TroubleHints onMicrophoneSettings={onMicrophoneSettings} onModelSettings={onModelSettings} />
      <footer className="flex items-center justify-between gap-6 border-t border-line pt-5">
        <BackButton onClick={onBack} />
        <div className="flex items-center gap-6">
          <span className="text-note text-muted">{t("onboarding.tryIt.later")}</span>
          <PrimaryButton onClick={onFinish}>{t("onboarding.tryIt.finish")}</PrimaryButton>
        </div>
      </footer>
    </>
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
    <details className="text-sm">
      <summary className="w-fit cursor-pointer rounded-md text-muted underline-offset-4 transition-colors duration-150 hover:text-fg hover:underline focus-visible:outline-2 focus-visible:outline-focus">
        {t("onboarding.tryIt.trouble.summary")}
      </summary>
      <ul className="mt-3 flex flex-col gap-3 rounded-xl bg-raised/50 px-4 py-3">
        <TroubleHint text={t("onboarding.tryIt.trouble.microphone")}>
          <TextButton onClick={onMicrophoneSettings}>
            {t("onboarding.tryIt.trouble.microphoneSettings")}
          </TextButton>
          <TextButton onClick={() => void commands.openMicrophonePrivacySettings()}>
            {t("onboarding.tryIt.trouble.privacySettings")}
          </TextButton>
        </TroubleHint>
        <TroubleHint text={t("onboarding.tryIt.trouble.model")}>
          <TextButton onClick={onModelSettings}>
            {t("onboarding.tryIt.trouble.modelSettings")}
          </TextButton>
        </TroubleHint>
        <TroubleHint text={t("onboarding.tryIt.trouble.other")}>
          <TextButton onClick={() => void commands.openLogFolder()}>
            {t("settings.logFolder.open")}
          </TextButton>
        </TroubleHint>
      </ul>
    </details>
  );
}

function TroubleHint({ text, children }: { text: string; children: ReactNode }) {
  return (
    <li className="flex flex-col gap-1">
      <span>{text}</span>
      <span className="flex flex-wrap gap-x-4 gap-y-1">{children}</span>
    </li>
  );
}
