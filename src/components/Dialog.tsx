import { type ReactNode, type RefObject, useEffect, useId, useRef } from "react";

import { Button } from "./Button";

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * The one modal primitive of the main window. While open it keeps Tab and Shift+Tab inside the
 * dialog, closes on Escape (and on a click on the backdrop unless `closeOnBackdrop` is false),
 * starts focus on `initialFocus` (or the first control), and hands focus back to whatever had
 * it before (normally the button that opened it) when it closes.
 */
export function Modal({
  role = "dialog",
  labelledBy,
  describedBy,
  onClose,
  closeOnBackdrop = true,
  initialFocus,
  className = "",
  children,
}: {
  role?: "dialog" | "alertdialog";
  labelledBy: string;
  describedBy?: string;
  onClose: () => void;
  closeOnBackdrop?: boolean;
  initialFocus?: RefObject<HTMLElement | null>;
  className?: string;
  children: ReactNode;
}) {
  const dialog = useRef<HTMLDivElement>(null);
  // The latest onClose without re-running the effect (and refocusing) on every render.
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  });

  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const controls = () => [...(dialog.current?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? [])];
    (initialFocus?.current ?? controls()[0] ?? dialog.current)?.focus();

    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        close.current();
        return;
      }
      if (event.key !== "Tab") return;
      const items = controls();
      const first = items[0];
      const last = items[items.length - 1];
      if (!first || !last) {
        event.preventDefault();
        return;
      }
      const active = document.activeElement;
      const inside = active instanceof Node && dialog.current?.contains(active);
      if (event.shiftKey && (active === first || !inside)) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (active === last || !inside)) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      if (opener?.isConnected) opener.focus();
    };
  }, [initialFocus]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-fg/25 p-6 transition-opacity duration-200 ease-out-strong starting:opacity-0">
      <div
        aria-hidden="true"
        data-testid="dialog-backdrop"
        className="absolute inset-0"
        onClick={() => {
          if (closeOnBackdrop) close.current();
        }}
      />
      <div
        ref={dialog}
        role={role}
        aria-modal="true"
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
        tabIndex={-1}
        className={`relative flex w-full max-w-sm flex-col gap-4 rounded-card border border-line bg-surface p-6 shadow-lg outline-none transition-[opacity,transform] duration-200 ease-out-strong origin-center starting:scale-[0.96] starting:opacity-0 motion-reduce:starting:scale-100 ${className}`}
      >
        {children}
      </div>
    </div>
  );
}

/**
 * A small confirmation of a destructive action. Focus starts on the safe choice, so Enter never
 * deletes by accident; Escape and the backdrop cancel.
 */
export function ConfirmDialog({
  title,
  body,
  confirm,
  cancel,
  onConfirm,
  onCancel,
}: {
  title: string;
  body: string;
  confirm: string;
  cancel: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const titleId = useId();
  const bodyId = useId();
  const cancelRef = useRef<HTMLButtonElement>(null);

  return (
    <Modal
      role="alertdialog"
      labelledBy={titleId}
      describedBy={bodyId}
      onClose={onCancel}
      initialFocus={cancelRef}
    >
      <h2 id={titleId} className="text-heading text-balance">
        {title}
      </h2>
      <p id={bodyId} className="text-muted">
        {body}
      </p>
      <div className="flex justify-end gap-2">
        <Button ref={cancelRef} variant="secondary" onClick={onCancel}>
          {cancel}
        </Button>
        <Button variant="danger" onClick={onConfirm}>
          {confirm}
        </Button>
      </div>
    </Modal>
  );
}
