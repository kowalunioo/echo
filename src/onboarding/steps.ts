/** The onboarding steps (settings-and-first-run.md rule 2), in order. */
export const ONBOARDING_STEPS = ["welcome", "microphone", "model", "tryIt"] as const;
export type OnboardingStep = (typeof ONBOARDING_STEPS)[number];

/** Whether Windows privacy settings let desktop apps use the Microphone. */
export type MicrophoneAccess = "allowed" | "denied";

export interface OnboardingFacts {
  /** The user moved past Welcome (the `onboardingWelcomeDone` setting). */
  welcomeDone: boolean;
  /** The latest Windows privacy check; `null` while the first check is running. */
  microphone: MicrophoneAccess | null;
  /** The user chose "Skip" on the Microphone access step in this session. */
  microphoneSkipped: boolean;
  /** A Model is active (Choose a Model is complete). */
  modelActive: boolean;
}

/**
 * The first incomplete step (rule 4): onboarding resumes here whenever the window opens. The
 * Microphone access step appears only while Windows blocks the Microphone (rule 2.2).
 */
export function currentStep(facts: OnboardingFacts): OnboardingStep {
  if (!facts.welcomeDone) return "welcome";
  if (facts.microphone === "denied" && !facts.microphoneSkipped) return "microphone";
  if (!facts.modelActive) return "model";
  return "tryIt";
}

/** The steps shown in the progress indicator: Microphone access only when it is needed. */
export function visibleSteps(microphoneNeeded: boolean): OnboardingStep[] {
  return ONBOARDING_STEPS.filter((step) => step !== "microphone" || microphoneNeeded);
}
