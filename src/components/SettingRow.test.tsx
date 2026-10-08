import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { SectionHeading } from "./SectionHeading";
import { SettingRow } from "./SettingRow";

describe("SettingRow", () => {
  it("shows the label, the description and the control", () => {
    render(
      <SettingRow label="Microphone" description="The input device Echo listens to.">
        <button type="button">Choose</button>
      </SettingRow>,
    );
    expect(screen.getByText("Microphone")).toBeInTheDocument();
    expect(screen.getByText("The input device Echo listens to.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Choose" })).toBeInTheDocument();
  });

  it("keeps the full text as a tooltip, since a long label or description is cut to one line", () => {
    render(
      <SettingRow label="Record Shortcut" description="Starts and stops a Recording.">
        <span />
      </SettingRow>,
    );
    expect(screen.getByText("Record Shortcut")).toHaveAttribute("title", "Record Shortcut");
    expect(screen.getByText("Starts and stops a Recording.")).toHaveAttribute(
      "title",
      "Starts and stops a Recording.",
    );
  });

  it("gives the label an id so the control can name itself by it", () => {
    render(
      <SettingRow label="Cancel Shortcut" labelId="cancel-label">
        <span />
      </SettingRow>,
    );
    expect(screen.getByText("Cancel Shortcut")).toHaveAttribute("id", "cancel-label");
  });
});

describe("SectionHeading", () => {
  it("is a second-level heading under the page title", () => {
    render(<SectionHeading>Shortcuts</SectionHeading>);
    expect(screen.getByRole("heading", { level: 2, name: "Shortcuts" })).toBeInTheDocument();
  });
});
