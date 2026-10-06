import { useEffect, useState } from "react";

import { commands } from "../bindings";

/**
 * The WAV file that replaces the Microphone in fake-microphone mode (dictation-pipeline.md
 * rules 40–41), or `null` in normal operation. The launch option cannot change while Echo runs,
 * so it is asked once per component.
 */
export function useTestAudio(): string | null {
  const [file, setFile] = useState<string | null>(null);
  useEffect(() => {
    let current = true;
    commands
      .getTestAudio()
      .then((name) => {
        if (current) setFile(name);
      })
      .catch((error: unknown) => {
        console.error("get_test_audio failed", error);
      });
    return () => {
      current = false;
    };
  }, []);
  return file;
}
