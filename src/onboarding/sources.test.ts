import { describe, expect, it } from "vitest";

import { backend } from "../test/backend";
import { checkMicrophoneAccess } from "./sources";

describe("checkMicrophoneAccess", () => {
  it("asks the backend for the Windows privacy check", async () => {
    expect(await checkMicrophoneAccess()).toBe("allowed");
    backend.microphoneAccess = "denied";
    expect(await checkMicrophoneAccess()).toBe("denied");
    expect(backend.commandsCalled("microphone_access")).toHaveLength(2);
  });

  it("does not block the onboarding when the check itself fails", async () => {
    backend.failing.set("microphone_access", "boom");
    expect(await checkMicrophoneAccess()).toBe("allowed");
  });
});
