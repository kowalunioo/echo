import { useEffect, useId, useRef } from "react";

/**
 * A small modal confirmation. Focus starts on the safe choice; Escape and the backdrop cancel.
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

  useEffect(() => {
    cancelRef.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [onCancel]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-fg/25 p-6">
      <div className="absolute inset-0" aria-hidden="true" onClick={onCancel} />
      <div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={bodyId}
        className="relative flex w-full max-w-sm flex-col gap-4 rounded-card border border-line bg-surface p-6 shadow-lg"
      >
        <h2 id={titleId} className="font-display text-lg font-semibold">
          {title}
        </h2>
        <p id={bodyId} className="text-muted">
          {body}
        </p>
        <div className="flex justify-end gap-2">
          <button
            ref={cancelRef}
            type="button"
            onClick={onCancel}
            className="rounded-lg border border-line px-4 py-1.5 text-sm font-medium hover:bg-raised"
          >
            {cancel}
          </button>
          <button
            type="button"
            onClick={onConfirm}
            className="rounded-lg bg-danger px-4 py-1.5 text-sm font-medium text-surface hover:opacity-90"
          >
            {confirm}
          </button>
        </div>
      </div>
    </div>
  );
}
