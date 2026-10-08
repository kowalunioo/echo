/**
 * A failure as the user reads it: one plain line (announced as an alert) and, below it, the
 * technical detail in smaller text that can be selected and copied into a bug report.
 */
export function FailureMessage({
  message,
  detail,
  className = "",
}: {
  message: string;
  /** Already formatted, e.g. "Details: out of memory"; omitted when there is none. */
  detail?: string | undefined;
  className?: string;
}) {
  return (
    <div className={`flex flex-col gap-0.5 ${className}`}>
      <p role="alert">{message}</p>
      {detail && (
        <p className="cursor-text text-note break-words text-muted select-text">{detail}</p>
      )}
    </div>
  );
}
