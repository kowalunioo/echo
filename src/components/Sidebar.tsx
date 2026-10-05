import { useTranslation } from "react-i18next";

import { PAGES, useShell } from "../store/shell";
import { EchoMark, PageIcon } from "./icons";

export function Sidebar() {
  const { t } = useTranslation();
  const page = useShell((s) => s.page);
  const setPage = useShell((s) => s.setPage);

  return (
    <aside className="flex w-60 shrink-0 flex-col border-r border-line bg-sidebar px-3 py-5">
      <div className="flex items-center gap-3 px-3 pb-6">
        <EchoMark className="size-8" />
        <div className="min-w-0">
          <p className="font-display text-base leading-tight font-semibold">{t("app.name")}</p>
          <p className="truncate text-xs text-muted">{t("app.tagline")}</p>
        </div>
      </div>

      <nav aria-label={t("nav.label")}>
        <ul className="flex flex-col gap-0.5">
          {PAGES.map((id) => {
            const current = id === page;
            return (
              <li key={id}>
                <button
                  type="button"
                  onClick={() => {
                    setPage(id);
                  }}
                  aria-current={current ? "page" : undefined}
                  className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition-colors duration-150 ${
                    current
                      ? "bg-raised font-medium text-fg"
                      : "text-muted hover:bg-raised/60 hover:text-fg"
                  }`}
                >
                  <span className={current ? "text-accent" : ""}>
                    <PageIcon page={id} />
                  </span>
                  {t(`nav.${id}`)}
                </button>
              </li>
            );
          })}
        </ul>
      </nav>

      <StatusArea />
    </aside>
  );
}

/** Active Model state, update and error notices (settings-and-first-run.md "UI"). */
function StatusArea() {
  const { t } = useTranslation();
  return (
    <section aria-label={t("status.label")} className="mt-auto px-3">
      <p className="flex items-center gap-2 rounded-lg border border-line bg-surface px-3 py-2 text-xs text-muted">
        <span className="size-2 rounded-full bg-muted/50" aria-hidden="true" />
        {t("status.noModel")}
      </p>
    </section>
  );
}
