import { commands } from "../bindings";
import type { MicrophoneAccess } from "./steps";

/*
 * Facts the onboarding needs from other slices. The ones still marked PLACEHOLDER have a fixed
 * answer; the owning slice replaces it with the real source and removes the placeholder controls
 * in Onboarding.tsx.
 */

/**
 * Whether Windows privacy settings let desktop apps use the Microphone (microphone.md rule 7).
 * If the check itself fails, the step is not shown: a Recording still reports a real block.
 */
export async function checkMicrophoneAccess(): Promise<MicrophoneAccess> {
  try {
    return await commands.microphoneAccess();
  } catch (error) {
    console.error("microphone_access failed", error);
    return "allowed";
  }
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
