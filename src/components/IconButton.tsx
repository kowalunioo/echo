import type { ButtonHTMLAttributes, ReactNode } from "react";

/**
 * A secondary action drawn as its icon alone, at the right end of a row. The label is both the
 * accessible name and the tooltip.
 */
export function IconButton({
  label,
  children,
  className = "",
  ...props
}: Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-label" | "title"> & {
  label: string;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={`grid size-7 shrink-0 place-items-center rounded-md text-muted transition-colors duration-150 hover:bg-raised hover:text-fg disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:bg-transparent disabled:hover:text-muted ${className}`}
      {...props}
    >
      {children}
    </button>
  );
}
