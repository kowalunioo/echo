import type { ReactNode } from "react";

/** A muted heading over a group of SettingRows. */
export function SectionHeading({ id, children }: { id?: string; children: ReactNode }) {
  return (
    <h2 id={id} className="truncate pb-1 text-sm font-semibold text-muted">
      {children}
    </h2>
  );
}
