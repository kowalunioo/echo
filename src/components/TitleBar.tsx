import { getCurrentWindow } from "@tauri-apps/api/window";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { EchoMark } from "./icons";

/**
 * Echo's own title bar: the main window has no native one (tauri.conf.json `decorations:
 * false`). The mark and name sit top-left; the Windows controls top-right. The bar is the drag
 * region, so double-clicking it maximizes and dragging it moves the window, as Windows does.
 *
 * `withSidebar` paints the left column in the sidebar's tone so the bar reads as part of it.
 */
export function TitleBar({ withSidebar = false }: { withSidebar?: boolean }) {
  const { t } = useTranslation();
  const act = (command: "minimize" | "toggleMaximize" | "close") => () => {
    void getCurrentWindow()[command]();
  };

  return (
    <header data-tauri-drag-region className="flex h-12 shrink-0 items-stretch bg-bg select-none">
      <div
        data-tauri-drag-region
        className={`flex w-68 shrink-0 items-center gap-2.5 px-5 pt-2 ${
          withSidebar ? "border-r border-line bg-sidebar" : ""
        }`}
      >
        <EchoMark className="pointer-events-none h-5 w-[18px]" />
        <span className="pointer-events-none text-sm font-semibold tracking-[0.01em]">
          {t("app.name")}
        </span>
      </div>
      <div data-tauri-drag-region className="flex flex-1 items-start justify-end">
        <WindowButton label={t("window.minimize")} onClick={act("minimize")}>
          <path d="M0.5 5.5h10" />
        </WindowButton>
        <WindowButton label={t("window.maximize")} onClick={act("toggleMaximize")}>
          <rect x="0.5" y="0.5" width="9" height="9" />
        </WindowButton>
        <WindowButton label={t("window.close")} onClick={act("close")} danger>
          <path d="m0.5 0.5 9 9M9.5 0.5l-9 9" />
        </WindowButton>
      </div>
    </header>
  );
}

/**
 * One Windows 11 caption button: 46 x 32 at the top edge, a 10px hairline glyph, a faint fill on
 * hover; the close one turns the Windows red with a white glyph.
 */
function WindowButton({
  label,
  danger = false,
  onClick,
  children,
}: {
  label: string;
  danger?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className={`flex h-8 w-[46px] items-center justify-center text-fg transition-colors duration-100 focus-visible:outline-offset-[-2px] ${
        danger ? "hover:bg-[#c42b1c] hover:text-white" : "hover:bg-fg/[0.07]"
      }`}
    >
      <svg
        viewBox="0 0 10 10"
        width="10"
        height="10"
        fill="none"
        stroke="currentColor"
        strokeWidth="1"
        shapeRendering={danger ? "auto" : "crispEdges"}
        aria-hidden="true"
      >
        {children}
      </svg>
    </button>
  );
}
