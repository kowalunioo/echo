import { type ReactNode, useEffect, useState } from "react";
import { Trans, useTranslation } from "react-i18next";

import { commands } from "../bindings";
import { EchoMark } from "../components/icons";
import { Placeholder } from "../components/PageView";
import { UiLanguagePicker } from "../components/UiLanguagePicker";
import { useSetting } from "../store/settings";
import { checkMicrophoneAccess, useModelActive, useRecordShortcut } from "./sources";
import { type MicrophoneAccess, type OnboardingStep, currentStep, visibleSteps } from "./steps";

/** How often the Microphone access step re-checks Windows privacy settings (rule 2.2). */
const MICROPHONE_RECHECK_MS = 2000;

/**
 * First-run onboarding (settings-and-first-run.md rules 1–4), shown in the main window until the
 * user presses "Finish". It always opens at the first incomplete step.
 */
export function Onboarding() {
  const { t } = useTranslation();
  const [welcomeDone, setWelcomeDone] = useSetting("onboardingWelcomeDone");
  const [, setCompleted] = useSetting("onboardingCompleted");
  const [microphoneSkipped, setMicrophoneSkipped] = useState(false);
  // PLACEHOLDER until Models (#12): lets the user reach "Try it" without an active Model.
  const [modelStepBypassed, setModelStepBypassed] = useState(false);
  const modelActive = useModelActive() || modelStepBypassed;
  const microphone = useMicrophoneAccess(welcomeDone && !microphoneSkipped);

  // Wait for the first privacy check rather than flash a step that is about to be replaced.
  const waiting = welcomeDone && !microphoneSkipped && microphone === null;
  const step = currentStep({ welcomeDone, microphone, microphoneSkipped, modelActive });

  return (
    <div className="flex h-full flex-col items-center overflow-y-auto px-6 py-10">
      <div className="flex w-full max-w-xl flex-col gap-8">
        <header className="flex items-center justify-between gap-6">
          <div className="flex items-center gap-3">
            <EchoMark className="size-8" />
            <span className="font-display text-base font-semibold">{t("app.name")}</span>
          </div>
          <Progress current={step} steps={visibleSteps(microphone === "denied")} />
        </header>

        {!waiting && (
          <section className="flex flex-col gap-6 rounded-card border border-line bg-surface p-8">
            {step === "welcome" && <WelcomeStep onContinue={() => void setWelcomeDone(true)} />}
            {step === "microphone" && (
              <MicrophoneStep
                onSkip={() => {
                  setMicrophoneSkipped(true);
                }}
              />
            )}
            {step === "model" && (
              <ModelStep
                onContinueWithout={() => {
                  setModelStepBypassed(true);
                }}
              />
            )}
            {step === "tryIt" && <TryItStep onFinish={() => void setCompleted(true)} />}
          </section>
        )}
      </div>
    </div>
  );
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
            <span
              aria-hidden="true"
              className={`size-1.5 rounded-full ${state === "next" ? "bg-muted/40" : "bg-accent"}`}
            />
            {t(`onboarding.steps.${step}`)}
          </li>
        );
      })}
    </ol>
  );
}

function StepHeader({ title, lead }: { title: string; lead: string }) {
  return (
    <header className="flex flex-col gap-2">
      <h1 className="font-display text-2xl font-semibold tracking-tight">{title}</h1>
      <p className="text-muted">{lead}</p>
    </header>
  );
}

function PrimaryButton({ children, onClick }: { children: ReactNode; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="shrink-0 rounded-lg bg-accent-strong px-5 py-2 whitespace-nowrap font-medium text-accent-fg transition-opacity duration-150 hover:opacity-90"
    >
      {children}
    </button>
  );
}

function TextButton({ children, onClick }: { children: ReactNode; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="rounded-md text-sm text-muted underline-offset-4 transition-colors duration-150 hover:text-fg hover:underline"
    >
      {children}
    </button>
  );
}

function WelcomeStep({ onContinue }: { onContinue: () => void }) {
  const { t } = useTranslation();
  return (
    <>
      <StepHeader title={t("onboarding.welcome.title")} lead={t("onboarding.welcome.lead")} />
      <ul className="flex flex-col gap-3">
        {(["local", "model", "shortcut"] as const).map((point) => (
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

function MicrophoneStep({ onSkip }: { onSkip: () => void }) {
  const { t } = useTranslation();
  return (
    <>
      <StepHeader title={t("onboarding.microphone.title")} lead={t("onboarding.microphone.lead")} />
      <p>{t("onboarding.microphone.howTo")}</p>
      <p aria-live="polite" className="flex items-center gap-2 text-sm text-muted">
        <span aria-hidden="true" className="size-2 animate-pulse rounded-full bg-accent" />
        {t("onboarding.microphone.checking")}
      </p>
      <footer className="flex items-center justify-between gap-6">
        <div className="flex flex-col items-start gap-0.5">
          <TextButton onClick={onSkip}>{t("onboarding.microphone.skip")}</TextButton>
          <span className="text-xs text-muted">{t("onboarding.microphone.skipNote")}</span>
        </div>
        <PrimaryButton onClick={() => void commands.openMicrophonePrivacySettings()}>
          {t("onboarding.microphone.open")}
        </PrimaryButton>
      </footer>
    </>
  );
}

function ModelStep({ onContinueWithout }: { onContinueWithout: () => void }) {
  const { t } = useTranslation();
  return (
    <>
      <StepHeader title={t("onboarding.model.title")} lead={t("onboarding.model.lead")} />
      {/* PLACEHOLDER until Models (#12): the Model list, download and "Use this Model". */}
      <Placeholder>{t("onboarding.model.upcoming")}</Placeholder>
      <footer className="flex justify-end">
        <TextButton onClick={onContinueWithout}>{t("onboarding.model.continueWithout")}</TextButton>
      </footer>
    </>
  );
}

function TryItStep({ onFinish }: { onFinish: () => void }) {
  const { t } = useTranslation();
  const shortcut = useRecordShortcut();
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
            kbd: (
              <kbd className="rounded-md border border-line bg-raised px-1.5 py-0.5 font-sans text-sm font-medium" />
            ),
          }}
        />
      </p>
      <label className="flex flex-col gap-2">
        <span className="text-sm font-medium">{t("onboarding.tryIt.fieldLabel")}</span>
        <textarea
          rows={4}
          placeholder={t("onboarding.tryIt.fieldPlaceholder")}
          className="resize-none rounded-xl border border-line bg-bg px-4 py-3 select-text placeholder:text-muted focus-visible:outline-2 focus-visible:outline-focus"
        />
      </label>
      <footer className="flex items-center justify-between gap-6">
        <span className="text-xs text-muted">{t("onboarding.tryIt.later")}</span>
        <PrimaryButton onClick={onFinish}>{t("onboarding.tryIt.finish")}</PrimaryButton>
      </footer>
    </>
  );
}
