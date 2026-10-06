import type { MicrophoneAccess } from "./steps";

/*
 * Facts the onboarding needs from slices that are not built yet. Each one is a clearly marked
 * PLACEHOLDER with a fixed answer; the owning slice replaces it with the real source and removes
 * the placeholder controls in Onboarding.tsx.
 */

/**
 * PLACEHOLDER until Microphone (#16): whether Windows privacy settings let desktop apps use the
 * Microphone. Always "allowed" for now, so the Microphone access step stays hidden.
 */
export function checkMicrophoneAccess(): Promise<MicrophoneAccess> {
  return Promise.resolve("allowed");
}

/**
 * PLACEHOLDER until Models (#12): whether a Model is active. Always false for now; the Choose a
 * Model step offers a temporary "Continue without a Model" link instead.
 */
export function useModelActive(): boolean {
  return false;
}

/**
 * PLACEHOLDER until Record Shortcut (#14): the Record Shortcut as shown to the user, and whether
 * it is held (Push-to-Talk Mode) or pressed (Toggle Mode). These are the spec defaults.
 */
export function useRecordShortcut(): { label: string; mode: "pushToTalk" | "toggle" } {
  return { label: "Ctrl+Space", mode: "pushToTalk" };
}
