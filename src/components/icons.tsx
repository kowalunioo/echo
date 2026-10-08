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

/** A counter-clockwise arrow: back to the default. */
export function ResetIcon() {
  return (
    <Icon width="16" height="16">
      <path d="M3 12a9 9 0 1 0 2.6-6.4L3 8" />
      <path d="M3 3v5h5" />
    </Icon>
  );
}

/** A pencil: this value can be edited. */
export function PencilIcon() {
  return (
    <Icon width="14" height="14">
      <path d="M4 20h4L19 9a2.8 2.8 0 0 0-4-4L4 16v4z" />
      <path d="m13.5 6.5 4 4" />
    </Icon>
  );
}

/**
 * Echo's mark without its app-icon tile: three bars on the brand's 32-unit grid. The outer bars
 * use the current text colour so they read on light and dark; the middle one is the accent.
 */
export function EchoMark({ className }: { className?: string }) {
  return (
    <svg viewBox="7 6 18 20" className={className} aria-hidden="true">
      <rect x="8" y="7" width="16" height="4" rx="2" fill="currentColor" />
      <rect x="8" y="14" width="11" height="4" rx="2" className="fill-accent" />
      <rect x="8" y="21" width="16" height="4" rx="2" fill="currentColor" />
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

export function CopyIcon() {
  return (
    <Icon width="16" height="16">
      <rect x="8.5" y="8.5" width="11" height="11" rx="2" />
      <path d="M15.5 8.5V6.5a2 2 0 0 0-2-2h-7a2 2 0 0 0-2 2v7a2 2 0 0 0 2 2h2" />
    </Icon>
  );
}

/** Re-insert: text going back into the application. */
export function ReinsertIcon() {
  return (
    <Icon width="16" height="16">
      <path d="M19 5v6.5a3 3 0 0 1-3 3H5.5" />
      <path d="m9.5 10.5-4 4 4 4" />
    </Icon>
  );
}

export function TrashIcon() {
  return (
    <Icon width="16" height="16">
      <path d="M4.5 7h15M9.5 7V5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v2" />
      <path d="m6.5 7 .8 11.2a2 2 0 0 0 2 1.8h5.4a2 2 0 0 0 2-1.8L17.5 7" />
    </Icon>
  );
}

/** An ✕: stop or dismiss. */
export function CloseIcon() {
  return (
    <Icon width="16" height="16">
      <path d="M6.5 6.5l11 11M17.5 6.5l-11 11" />
    </Icon>
  );
}

/** An ⓘ in front of an explanatory note. */
export function InfoIcon({ className }: { className?: string }) {
  return (
    <Icon width="16" height="16" className={className}>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 11v5.5M12 7.75v.25" strokeWidth="1.9" />
    </Icon>
  );
}

/** A padlock: stays on this computer (Welcome). */
export function LockIcon({ className }: { className?: string }) {
  return (
    <Icon width="16" height="16" className={className}>
      <rect x="5" y="10.5" width="14" height="10" rx="2" />
      <path d="M8.5 10.5V7.5a3.5 3.5 0 0 1 7 0v3" />
    </Icon>
  );
}

/** An arrow into a tray: download. */
export function DownloadIcon({ className }: { className?: string }) {
  return (
    <Icon width="16" height="16" className={className}>
      <path d="M12 4v11M7.5 10.5 12 15l4.5-4.5M5 19.5h14" />
    </Icon>
  );
}

/** A keyboard: the Record Shortcut (Welcome). */
export function KeyboardIcon({ className }: { className?: string }) {
  return (
    <Icon width="16" height="16" className={className}>
      <rect x="2.5" y="6" width="19" height="12" rx="2" />
      <path d="M7 14.5h10M6.5 10h.01M10 10h.01M13.5 10h.01M17 10h.01" />
    </Icon>
  );
}

/** → in front of an action that opens another page of Echo. */
export function ArrowIcon() {
  return (
    <Icon width="16" height="16">
      <path d="M5 12h14M13 6l6 6-6 6" />
    </Icon>
  );
}

/** A circled "!" in front of a problem: the ⓘ turned upside down. */
export function AlertIcon({ className }: { className?: string }) {
  return (
    <Icon width="16" height="16" className={className}>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 7.5v5.5M12 16.25v.25" strokeWidth="1.9" />
    </Icon>
  );
}

/** The drop-down marker of a picker button. */
export function ChevronIcon() {
  return (
    <Icon width="16" height="16" strokeWidth="1.8" className="shrink-0 text-muted">
      <path d="m6 9 6 6 6-6" />
    </Icon>
  );
}

export function MinusIcon() {
  return (
    <Icon width="14" height="14">
      <path d="M6 12h12" />
    </Icon>
  );
}

export function PlusIcon() {
  return (
    <Icon width="14" height="14">
      <path d="M6 12h12M12 6v12" />
    </Icon>
  );
}

/** A completed step. */
export function CheckIcon({ className }: { className?: string }) {
  return (
    <Icon width="12" height="12" strokeWidth="2.4" className={className}>
      <path d="m5 12.5 4.5 4.5L19 7.5" />
    </Icon>
  );
}

/** A clockwise arrow: look again (Check for updates). */
export function RefreshIcon() {
  return (
    <Icon width="15" height="15">
      <path d="M21 12a9 9 0 1 1-2.6-6.4L21 8" />
      <path d="M21 3v5h-5" />
    </Icon>
  );
}

/** A folder: opens a folder in File Explorer. */
export function FolderIcon() {
  return (
    <Icon width="15" height="15">
      <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
    </Icon>
  );
}

/** An arrow out of a box: opens something outside Echo (a Windows settings page). */
export function ExternalIcon() {
  return (
    <Icon width="15" height="15">
      <path d="M14 4h6v6" />
      <path d="M20 4l-9 9" />
      <path d="M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4" />
    </Icon>
  );
}
