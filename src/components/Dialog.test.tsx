import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { ConfirmDialog } from "./Dialog";

function Harness({ onConfirm = () => undefined }: { onConfirm?: () => void }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        type="button"
        onClick={() => {
          setOpen(true);
        }}
      >
        Delete it
      </button>
      {open && (
        <ConfirmDialog
          title="Delete it?"
          body="This can't be undone."
          confirm="Delete"
          cancel="Keep"
          onConfirm={() => {
            setOpen(false);
            onConfirm();
          }}
          onCancel={() => {
            setOpen(false);
          }}
        />
      )}
    </>
  );
}

async function openDialog() {
  const user = userEvent.setup();
  render(<Harness />);
  await user.click(screen.getByRole("button", { name: "Delete it" }));
  return user;
}

describe("ConfirmDialog", () => {
  it("starts on the safe choice", async () => {
    await openDialog();

    expect(screen.getByRole("alertdialog", { name: "Delete it?" })).toHaveAccessibleDescription(
      "This can't be undone.",
    );
    expect(screen.getByRole("button", { name: "Keep" })).toHaveFocus();
  });

  it("keeps Tab and Shift+Tab inside the dialog", async () => {
    const user = await openDialog();

    await user.tab();
    expect(screen.getByRole("button", { name: "Delete" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "Keep" })).toHaveFocus();
    await user.tab({ shift: true });
    expect(screen.getByRole("button", { name: "Delete" })).toHaveFocus();
  });

  it("closes on Escape and returns focus to the button that opened it", async () => {
    const user = await openDialog();

    await user.keyboard("{Escape}");

    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delete it" })).toHaveFocus();
  });

  it("closes on a click on the backdrop", async () => {
    const user = await openDialog();

    await user.click(screen.getByTestId("dialog-backdrop"));

    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delete it" })).toHaveFocus();
  });

  it("confirms only on the confirm button", async () => {
    const onConfirm = vi.fn();
    const user = userEvent.setup();
    render(<Harness onConfirm={onConfirm} />);
    await user.click(screen.getByRole("button", { name: "Delete it" }));

    await user.click(screen.getByRole("button", { name: "Delete" }));

    expect(onConfirm).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: "Delete it" })).toHaveFocus();
  });
});
