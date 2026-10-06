import { type RefObject, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { type OverlayMessage, type OverlayView, commands, events } from "../bindings";
import { changeUiLanguage } from "../i18n";
import { useSettings } from "../store/settings";
import { SILENT_BARS, formatElapsed, nextBars } from "./meter";

const HIDDEN: OverlayView = { kind: "hidden" };
/** How long the pill takes to change width; the click region shrinks only after it. */
const WIDTH_TRANSITION_MS = 220;

interface Frame {
  bars: readonly number[];
  elapsedMs: number;
}
const FIRST_FRAME: Frame = { bars: SILENT_BARS, elapsedMs: 0 };

/**
 * The Overlay window's content (overlay.md): a rounded pill that shows getting ready, listening
 * (level meter, timer, cancel), transcribing (spinner, label, cancel) or a short message. The
 * backend decides what to show and when the window appears; this draws it, fades it in and out
 * and tells the backend the pill's size so clicks beside it reach the windows beneath.
 */
export function OverlayApp() {
  const view = useOverlayView();
  const frame = useFrames(view);
  const uiLanguage = useSettings((s) => s.settings?.uiLanguage);
  const position = useSettings((s) => s.settings?.overlayPosition ?? "bottom");

  // Rule 18: the Overlay follows the UI Language as it changes.
  useEffect(() => {
    if (uiLanguage) void changeUiLanguage(uiLanguage);
  }, [uiLanguage]);

  // While fading out, keep drawing what was last shown (rule 6).
  const visible = view.kind !== "hidden";
  const [shown, setShown] = useState<OverlayView>(view);
  if (visible && shown !== view) setShown(view);

  const pill = useRef<HTMLDivElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const style = usePillShape(pill, content);

  return (
    <div className="flex h-full items-center justify-center">
      <div
        ref={pill}
        data-testid="overlay-pill"
        data-visible={visible}
        data-position={position}
        style={style}
        className="overlay-pill h-full overflow-hidden rounded-full border border-line bg-surface text-fg"
      >
        <div ref={content} className="inline-flex h-full w-max items-center">
          <Content view={shown} frame={frame} />
        </div>
      </div>
    </div>
  );
}

function Content({ view, frame }: { view: OverlayView; frame: Frame }) {
  const { t } = useTranslation();
  switch (view.kind) {
    case "hidden":
      return null;
    case "gettingReady":
    case "listening": {
      const ready = view.kind === "listening";
      return (
        <div className="flex w-64 items-center gap-3 pr-1.5 pl-4">
          <span role="status" className="sr-only">
            {t(ready ? "overlay.listening" : "overlay.gettingReady")}
          </span>
          <Dot tone={ready ? "accent" : "muted"} pulsing={!ready} />
          <Bars bars={ready ? frame.bars : SILENT_BARS} muted={!ready} />
          <span className="text-[13px] text-muted tabular-nums">
            {formatElapsed(frame.elapsedMs)}
          </span>
          <CancelButton />
        </div>
      );
    }
    case "transcribing":
      return (
        <div className={`flex items-center gap-2.5 pl-4 ${view.cancellable ? "pr-1.5" : "pr-5"}`}>
          <Spinner />
          <span role="status" className="text-[13px] font-medium">
            {t("overlay.transcribing")}
          </span>
          {view.cancellable && <CancelButton />}
        </div>
      );
    case "message":
      return <Message message={view.message} actionable={view.actionable} />;
  }
}

function Message({ message, actionable }: { message: OverlayMessage; actionable: boolean }) {
  const { t } = useTranslation();
  const text =
    message.kind === "problem"
      ? t(`overlay.messages.${message.problem}`)
      : t("overlay.messages.microphoneFallback");
  const body = (
    <>
      <Dot tone={message.kind === "problem" ? "danger" : "accent"} />
      <span className="max-w-[440px] truncate text-[13px] font-medium">{text}</span>
    </>
  );
  if (!actionable) {
    return (
      <div role="alert" className="flex items-center gap-2.5 px-5">
        {body}
      </div>
    );
  }
  return (
    <button
      type="button"
      role="alert"
      onMouseDown={(e) => {
        e.preventDefault();
      }}
      onClick={() => void commands.overlayMessageClicked()}
      className="flex h-full cursor-pointer items-center gap-2.5 px-5 hover:bg-raised"
    >
      {body}
      <svg aria-hidden viewBox="0 0 16 16" className="size-3.5 text-muted">
        <path
          d="M6 3.5 10.5 8 6 12.5"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    </button>
  );
}

function Dot({ tone, pulsing }: { tone: "accent" | "muted" | "danger"; pulsing?: boolean }) {
  const colour = { accent: "bg-accent", muted: "bg-muted", danger: "bg-danger" }[tone];
  return (
    <span
      aria-hidden
      className={`size-2 shrink-0 rounded-full ${colour} ${pulsing ? "animate-pulse" : ""}`}
    />
  );
}

function Bars({ bars, muted }: { bars: readonly number[]; muted: boolean }) {
  return (
    <span
      aria-hidden
      data-testid="overlay-meter"
      className={`flex h-[22px] flex-1 items-center justify-center gap-1 ${muted ? "animate-pulse" : ""}`}
    >
      {bars.map((height, i) => (
        <span
          key={i}
          className={`w-1 rounded-full ${muted ? "bg-muted/50" : "bg-accent"}`}
          style={{ height: `${String(Math.round(height * 100))}%` }}
        />
      ))}
    </span>
  );
}

function Spinner() {
  return (
    <svg aria-hidden viewBox="0 0 16 16" className="size-4 shrink-0 animate-spin text-accent">
      <circle
        cx="8"
        cy="8"
        r="6"
        fill="none"
        stroke="currentColor"
        strokeOpacity="0.25"
        strokeWidth="2"
      />
      <path
        d="M8 2a6 6 0 0 1 6 6"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
      />
    </svg>
  );
}

/** Cancellation, like the Cancel Shortcut (rule 10). Never takes focus inside the page either. */
function CancelButton() {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      aria-label={t("overlay.cancel")}
      title={t("overlay.cancel")}
      onMouseDown={(e) => {
        e.preventDefault();
      }}
      onClick={() => void commands.overlayCancel()}
      className="grid size-7 shrink-0 cursor-pointer place-items-center rounded-full text-muted hover:bg-raised hover:text-fg"
    >
      <svg aria-hidden viewBox="0 0 16 16" className="size-3.5">
        <path
          d="M4 4l8 8M12 4l-8 8"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
        />
      </svg>
    </button>
  );
}

/** The backend's view: fetched once, then followed. A change that arrives first wins. */
function useOverlayView(): OverlayView {
  const [view, setView] = useState<OverlayView>(HIDDEN);
  useEffect(() => {
    let live = true;
    let changed = false;
    const stop = events.overlayViewChanged.listen((event) => {
      changed = true;
      setView(event.payload);
    });
    commands
      .getOverlayView()
      .then((current) => {
        if (live && !changed) setView(current);
      })
      .catch((error: unknown) => {
        console.error("get_overlay_view failed", error);
      });
    return () => {
      live = false;
      void stop.then((unlisten) => {
        unlisten();
      });
    };
  }, []);
  return view;
}

/** Level-meter and timer frames while recording; a new Recording starts from silence. */
function useFrames(view: OverlayView): Frame {
  const [frame, setFrame] = useState<Frame>(FIRST_FRAME);
  const recording = view.kind === "gettingReady" || view.kind === "listening";
  const [wasRecording, setWasRecording] = useState(recording);
  if (recording !== wasRecording) {
    setWasRecording(recording);
    if (recording) setFrame(FIRST_FRAME);
  }
  useEffect(() => {
    const stop = events.overlayFrame.listen((event) => {
      setFrame((previous) => ({
        bars: nextBars(previous.bars, event.payload.level ?? 0),
        elapsedMs: event.payload.elapsedMs,
      }));
    });
    return () => {
      void stop.then((unlisten) => {
        unlisten();
      });
    };
  }, []);
  return frame;
}

/**
 * Animates the pill's width to its content's and reports the pill's size to the backend, which
 * clips the window to it (rule 11). The region grows at once and shrinks after the animation.
 */
function usePillShape(
  pill: RefObject<HTMLDivElement | null>,
  content: RefObject<HTMLDivElement | null>,
) {
  const [width, setWidth] = useState<number | null>(null);

  useLayoutEffect(() => {
    const element = content.current;
    if (!element || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => {
      setWidth(element.offsetWidth);
    });
    observer.observe(element);
    return () => {
      observer.disconnect();
    };
  }, [content]);

  const reported = useRef(0);
  useEffect(() => {
    const height = pill.current?.offsetHeight ?? 0;
    if (width === null || height === 0) return;
    const target = width + 2;
    const report = () => {
      reported.current = target;
      void commands.overlayShape(target, height);
    };
    if (target >= reported.current) {
      report();
      return;
    }
    const timer = setTimeout(report, WIDTH_TRANSITION_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [pill, width]);

  // The border adds 2 px around the content.
  return width === null ? undefined : { width: width + 2 };
}
