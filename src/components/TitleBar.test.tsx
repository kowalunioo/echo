import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { TitleBar } from "./TitleBar";

const controls = { minimize: vi.fn(), toggleMaximize: vi.fn(), close: vi.fn() };
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => controls,
}));

describe("TitleBar", () => {
  it("shows Echo's mark and name and drives the window through its three controls", async () => {
    render(<TitleBar />);
    expect(screen.getByText("Echo")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Minimize" }));
    await userEvent.click(screen.getByRole("button", { name: "Maximize" }));
    await userEvent.click(screen.getByRole("button", { name: "Close window" }));
    expect(controls.minimize).toHaveBeenCalledOnce();
    expect(controls.toggleMaximize).toHaveBeenCalledOnce();
    expect(controls.close).toHaveBeenCalledOnce();
  });

  it("is a drag region for moving the window", () => {
    render(<TitleBar />);
    expect(screen.getByRole("banner")).toHaveAttribute("data-tauri-drag-region");
  });
});
