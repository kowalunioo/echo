import { describe, expect, it } from "vitest";

import { backend } from "../test/backend";
import { useSettings } from "./settings";

describe("settings store", () => {
  it("loads the settings from the backend", async () => {
    backend.settings.uiLanguage = "pl";

    await useSettings.getState().load();

    expect(useSettings.getState().status).toBe("ready");
    expect(useSettings.getState().settings?.uiLanguage).toBe("pl");
  });

  it("saves a change through update_settings with only that setting", async () => {
    await useSettings.getState().load();

    await useSettings.getState().set("uiLanguage", "pl");

    expect(backend.commandsCalled("update_settings").map((c) => c.args)).toEqual([
      { patch: { uiLanguage: "pl" } },
    ]);
    expect(backend.settings.uiLanguage).toBe("pl");
    expect(useSettings.getState().settings?.uiLanguage).toBe("pl");
  });

  it("shows a change at once, before the backend answers", async () => {
    await useSettings.getState().load();
    backend.hanging.add("update_settings");

    void useSettings.getState().set("uiLanguage", "pl");

    expect(useSettings.getState().settings?.uiLanguage).toBe("pl");
  });

  it("follows changes made elsewhere", async () => {
    await useSettings.getState().load();

    backend.changeSettings({ uiLanguage: "pl" });

    expect(useSettings.getState().settings?.uiLanguage).toBe("pl");
  });

  it("returns to the backend's settings when a change is refused", async () => {
    await useSettings.getState().load();
    backend.failing.set("update_settings", "invalid value");

    await useSettings.getState().set("uiLanguage", "pl");

    expect(useSettings.getState().settings?.uiLanguage).toBe("en");
  });

  it("resets a setting to its default", async () => {
    backend.settings.uiLanguage = "pl";
    await useSettings.getState().load();

    await useSettings.getState().reset("uiLanguage");

    expect(backend.commandsCalled("reset_setting")[0]?.args).toEqual({ key: "uiLanguage" });
    expect(useSettings.getState().settings?.uiLanguage).toBe("en");
  });

  it("reports an error when the settings cannot be loaded", async () => {
    backend.failing.set("get_settings", new Error("boom"));

    await useSettings.getState().load();

    expect(useSettings.getState().status).toBe("error");
  });
});
