import type { ReactNode } from "react";

/**
 * One key of a shortcut, the same everywhere. `inline` sits in a sentence; `small` sits in a
 * field or the sidebar next to other key caps. Used directly or as the `kbd` element of a
 * `<Trans>` string.
 */
export function Keycap({
  size = "inline",
  children,
}: {
  size?: "inline" | "small";
  children?: ReactNode;
}) {
  return (
    <kbd
      className={`rounded-md border border-line bg-raised font-sans font-medium text-fg shadow-[0_1px_0_var(--color-line)] ${
        size === "inline" ? "px-1.5 py-0.5 text-sm" : "px-1.5 text-xs leading-[18px]"
      }`}
    >
      {children}
    </kbd>
  );
}
