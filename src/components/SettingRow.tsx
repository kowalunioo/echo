import type { ReactNode } from "react";

/**
 * One setting on a page: label and description on the left, its control on the right, a hairline
 * under it except after the last row of a group. Label and description stay on one line each and
 * end in an ellipsis when they do not fit, with the full text as a tooltip. `below` holds anything
 * that belongs to the row but needs the full width, such as a failure message.
 */
export function SettingRow({
  label,
  description,
  labelId,
  below,
  children,
}: {
  label: string;
  description?: string;
  /** For a control that names itself with aria-labelledby. */
  labelId?: string;
  below?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-2 border-b border-line py-3.5 last:border-b-0">
      <div className="flex items-center justify-between gap-8">
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span id={labelId} title={label} className="truncate font-medium">
            {label}
          </span>
          {description && (
            <span title={description} className="truncate text-note text-muted">
              {description}
            </span>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-3">{children}</div>
      </div>
      {below}
    </div>
  );
}
