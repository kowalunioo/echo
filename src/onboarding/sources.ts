import { commands } from "../bindings";
import { useKeyLabel } from "../shortcut/keyLabel";
import { useModels } from "../store/models";
import { DEFAULT_RECORD_SHORTCUT } from "../store/recordShortcut";
import { useSettings } from "../store/settings";
import type { MicrophoneAccess } from "./steps";

/* Facts the onboarding and the History empty state need from other slices. */

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

/** Whether a Model is active (models.md rule 17): the Choose a Model step is then complete. */
export function useModelActive(): boolean {
  return useModels((s) => s.state?.active != null);
}

/**
 * The Record Shortcut as shown to the user, in the UI Language, and whether it is held
 * (Push-to-Talk Mode) or pressed (Toggle Mode) — from the settings.
 */
export function useRecordShortcut(): { label: string; mode: "pushToTalk" | "toggle" } {
  const combination = useSettings((s) => s.settings?.recordShortcut ?? DEFAULT_RECORD_SHORTCUT);
  const mode = useSettings((s) => s.settings?.shortcutMode ?? "pushToTalk");
  const label = useKeyLabel();
  return { label: label(combination), mode };
}
