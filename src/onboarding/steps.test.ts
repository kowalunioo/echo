import { describe, expect, it } from "vitest";

import { type OnboardingFacts, currentStep, visibleSteps } from "./steps";

const fresh: OnboardingFacts = {
  welcomeDone: false,
  microphone: "allowed",
  microphoneSkipped: false,
  modelActive: false,
};

describe("currentStep", () => {
  it("starts with Welcome", () => {
    expect(currentStep(fresh)).toBe("welcome");
    expect(currentStep({ ...fresh, microphone: "denied" })).toBe("welcome");
  });

  it("skips Microphone access when Windows allows the Microphone", () => {
    expect(currentStep({ ...fresh, welcomeDone: true })).toBe("model");
  });

  it("shows Microphone access while Windows blocks the Microphone, unless skipped", () => {
    const blocked = { ...fresh, welcomeDone: true, microphone: "denied" as const };
    expect(currentStep(blocked)).toBe("microphone");
    expect(currentStep({ ...blocked, microphoneSkipped: true })).toBe("model");
  });

  it("goes to Try it once a Model is active", () => {
    expect(currentStep({ ...fresh, welcomeDone: true, modelActive: true })).toBe("tryIt");
  });

  it("resumes at the first incomplete step", () => {
    // A Model became active (e.g. a background download finished) but Welcome was never left.
    expect(currentStep({ ...fresh, modelActive: true })).toBe("welcome");
  });
});

describe("visibleSteps", () => {
  it("includes Microphone access only when it is needed", () => {
    expect(visibleSteps(false)).toEqual(["welcome", "model", "tryIt"]);
    expect(visibleSteps(true)).toEqual(["welcome", "microphone", "model", "tryIt"]);
  });
});
