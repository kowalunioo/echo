import { describe, expect, it } from "vitest";

import {
  type OnboardingFacts,
  currentStep,
  nextStep,
  previousStep,
  shownStep,
  visibleSteps,
} from "./steps";

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

describe("Back and forward (rule 2a)", () => {
  const steps = visibleSteps(true);

  it("goes back to the previous shown step, and not before Welcome", () => {
    expect(previousStep("tryIt", steps)).toBe("model");
    expect(previousStep("model", steps)).toBe("microphone");
    expect(previousStep("model", visibleSteps(false))).toBe("welcome");
    expect(previousStep("welcome", steps)).toBeNull();
  });

  it("shows a revisited step only while it lies before the current step", () => {
    expect(shownStep("tryIt", "model", steps)).toBe("model");
    expect(shownStep("model", null, steps)).toBe("model");
    expect(shownStep("model", "tryIt", steps)).toBe("model");
    // Microphone access disappears once Windows allows it.
    expect(shownStep("model", "microphone", visibleSteps(false))).toBe("model");
  });

  it("moves forward from a revisited step, and back to following the current step at the end", () => {
    expect(nextStep("welcome", "tryIt", steps)).toBe("microphone");
    expect(nextStep("model", "tryIt", steps)).toBeNull();
  });
});
