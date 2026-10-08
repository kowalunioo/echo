import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import type { Page } from "../store/shell";

/** A page's title and description, with optional controls on the right (e.g. the History limit). */
export function PageHeader({ page, children }: { page: Page; children?: ReactNode }) {
  const { t } = useTranslation();
  return (
    <header className="flex items-start justify-between gap-6">
      <div className="flex min-w-0 flex-col gap-1">
        <h1 className="font-display text-xl font-semibold tracking-[-0.01em] text-balance">
          {t(`pages.${page}.title`)}
        </h1>
        <p className="text-pretty text-muted">{t(`pages.${page}.description`)}</p>
      </div>
      {children}
    </header>
  );
}
