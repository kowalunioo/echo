import type { ReactNode, SVGProps } from "react";

import type { Page } from "../store/shell";

function Icon({ children, ...props }: SVGProps<SVGSVGElement> & { children: ReactNode }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="18"
      height="18"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      {...props}
    >
      {children}
    </svg>
  );
}

export function EchoMark({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 32 32" className={className} aria-hidden="true">
      <rect width="32" height="32" rx="8" className="fill-accent" />
      <g fill="none" className="stroke-accent-fg" strokeLinecap="round" strokeWidth="2">
        <path d="M12.2 11.8a6 6 0 0 0 0 8.4" />
        <path d="M19.8 11.8a6 6 0 0 1 0 8.4" />
      </g>
      <circle cx="16" cy="16" r="2.2" className="fill-accent-fg" />
    </svg>
  );
}

const pageIcons: Record<Page, ReactNode> = {
  dictation: (
    <Icon>
      <rect x="9" y="3" width="6" height="11" rx="3" />
      <path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21" />
    </Icon>
  ),
  model: (
    <Icon>
      <path d="M12 3 4 7.5l8 4.5 8-4.5L12 3Z" />
      <path d="m4 12 8 4.5 8-4.5M4 16.5 12 21l8-4.5" />
    </Icon>
  ),
  vocabulary: (
    <Icon>
      <path d="M5 4.5A1.5 1.5 0 0 1 6.5 3H19v15H6.5A1.5 1.5 0 0 0 5 19.5v-15Z" />
      <path d="M5 19.5A1.5 1.5 0 0 0 6.5 21H19M9 7.5h6" />
    </Icon>
  ),
  history: (
    <Icon>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 7.5V12l3 2" />
    </Icon>
  ),
  app: (
    <Icon>
      <path d="M4 7h10M18 7h2M4 17h4M12 17h8" />
      <circle cx="16" cy="7" r="2" />
      <circle cx="10" cy="17" r="2" />
    </Icon>
  ),
};

export function PageIcon({ page }: { page: Page }) {
  return pageIcons[page];
}
