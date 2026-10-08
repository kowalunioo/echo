import {
  type CSSProperties,
  type RefObject,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";

import { type OverlayMessage, type OverlayView, commands, events } from "../bindings";
import { TestAudioMarker } from "../components/TestAudioMarker";
import { EchoMark } from "../components/icons";
import { changeUiLanguage } from "../i18n";
import { useSettings } from "../store/settings";
import { BAR_COUNT, SILENT_BARS, nextBars } from "./meter";

const HIDDEN: OverlayView = { kind: "hidden" };
/** How long the pill takes to change shape; the click region shrinks only after it. */
const SHAPE_TRANSITION_MS = 220;
/** The pill shrinks into a capsule, then the capsule splits into the dots (overlay.md rule 4). */
const DOTS_SETTLED_MS = SHAPE_TRANSITION_MS + 170;
/** The region the three dots need, bounce and glow included (see `.overlay-dot` in styles.css). */
const DOTS_SHAPE = { width: 64, height: 30 };

/**
 * The Overlay window's content (overlay.md): a rounded pill that shows getting ready, listening
 * (level meter and cancel) or a short message, and turns into three bouncing dots while
 * transcribing. The backend decides what to show and when the window appears; this draws it,
 * fades it in and out and tells the backend the shape's size so clicks beside it reach the
 * windows beneath.
 */
export function OverlayApp() {
  const view = useOverlayView();
  const bars = useBars(view);
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
  // While the pill folds into the dots, it keeps the content it had so that content can fade.
  const [body, setBody] = useState<OverlayView>(shown);
  if (shown.kind !== "transcribing" && body !== shown) setBody(shown);
  // A message that follows the dots fades in once the pill has grown back (styles.css).
  const [phases, setPhases] = useState({ current: shown.kind, previous: shown.kind });
  if (phases.current !== shown.kind) setPhases({ current: shown.kind, previous: phases.current });

  const shape = useRef<HTMLDivElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const folding = shown.kind === "transcribing";
  const style = usePillShape(shape, content, folding);

  return (
    <div className="flex h-full items-center justify-center">
      <div
        data-testid="overlay-pill"
        data-visible={visible}
        data-position={position}
        data-phase={shown.kind}
        data-previous={phases.previous}
        className="overlay-pill relative flex h-full items-center justify-center"
      >
        <div ref={shape} style={style} className="overlay-shape bg-surface text-fg">
          <span data-testid="overlay-edge" aria-hidden className="overlay-edge">
            <span className="overlay-shine" />
          </span>
          {/* While folding into the dots, what fades out is neither announced nor clickable. */}
          <div
            ref={content}
            aria-hidden={folding}
            inert={folding}
            className="overlay-content relative inline-flex h-full w-max items-center"
          >
            {/* dictation-pipeline.md rule 41: a fake-microphone session is marked here too. */}
            {body.kind !== "hidden" && (
              <span className="flex shrink-0 empty:hidden pl-3">
                <TestAudioMarker compact />
              </span>
            )}
            <Content view={body} bars={bars} />
          </div>
        </div>
        <Dots transcribing={folding} />
      </div>
    </div>
  );
}

function Content({ view, bars }: { view: OverlayView; bars: readonly number[] }) {
  const { t } = useTranslation();
  switch (view.kind) {
    case "hidden":
    case "transcribing":
      return null;
    case "gettingReady":
    case "listening": {
      const live = view.kind === "listening";
      return (
        <div className="flex w-[200px] items-center gap-3 pr-2 pl-[17px]">
          <Mark live={live} />
          <span className="relative flex h-[22px] flex-1 items-center">
            {/* On screen until audio flows (rule 2), then it fades as the meter fades in. */}
            <span
              aria-hidden
              data-testid="overlay-label"
              data-shown={!live}
              className="overlay-label absolute inset-x-0 truncate text-note font-medium text-muted"
            >
              {t("overlay.gettingReady")}
            </span>
            {live && <Bars bars={bars} />}
          </span>
          {/* One live region for both recording states. */}
          <span role="status" className="sr-only">
            {t(live ? "overlay.listening" : "overlay.gettingReady")}
          </span>
          <CancelButton />
        </div>
      );
    }
    case "message":
      return <Message message={view.message} actionable={view.actionable} />;
  }
}

/** Transcribing (rule 4): the pill has become three dots that bounce and light up in turn. */
function Dots({ transcribing }: { transcribing: boolean }) {
  const { t } = useTranslation();
  return (
    <span aria-hidden={!transcribing} className="overlay-dots">
      <span data-testid="overlay-dots" className="contents">
        {[-1, 0, 1].map((place) => (
          <span key={place} className="overlay-dot" style={{ "--place": place } as CSSProperties}>
            <span className="overlay-dot-body">
              <span className="overlay-edge" />
            </span>
          </span>
        ))}
      </span>
      {transcribing && (
        <span role="status" className="sr-only">
          {t("overlay.transcribing")}
        </span>
      )}
    </span>
  );
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
      {/* The text is the alert, so a clickable message stays a button for assistive tech. */}
      <span role="alert" className="max-w-[440px] truncate text-note font-medium">
        {text}
      </span>
    </>
  );
  if (!actionable) {
    return <div className="flex items-center gap-2.5 px-5">{body}</div>;
  }
  return (
    <button
      type="button"
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

function Dot({ tone }: { tone: "accent" | "danger" }) {
  const colour = { accent: "bg-accent", danger: "bg-danger" }[tone];
  return <span aria-hidden className={`size-2 shrink-0 rounded-full ${colour}`} />;
}

/**
 * Echo's three bars as the recording states' status mark: muted and pulsing while getting ready,
 * in full colour once the Microphone is live.
 */
function Mark({ live }: { live: boolean }) {
  return (
    <span
      aria-hidden
      data-testid="overlay-mark"
      data-live={live}
      className={`flex shrink-0 transition-[opacity,filter] duration-200 ${live ? "" : "animate-pulse opacity-60 grayscale"}`}
    >
      <EchoMark className="h-4 w-3.5" />
    </span>
  );
}

function Bars({ bars }: { bars: readonly number[] }) {
  return (
    <span
      aria-hidden
      data-testid="overlay-meter"
      className="overlay-meter absolute inset-0 flex items-center justify-center gap-1"
    >
      {bars.map((height, i) => (
        <span
          key={i}
          className="overlay-bar h-full w-[3px] rounded-full bg-accent"
          style={{ transform: `scaleY(${height.toFixed(3)})` }}
        />
      ))}
    </span>
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
      className="grid size-7 shrink-0 cursor-pointer place-items-center rounded-full text-muted transition-[background-color,color,transform] duration-150 ease-out-strong hover:bg-raised hover:text-fg active:scale-95"
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

/** Level-meter frames while recording; a new Recording starts from silence. */
function useBars(view: OverlayView): readonly number[] {
  const [bars, setBars] = useState<readonly number[]>(SILENT_BARS);
  const recording = view.kind === "gettingReady" || view.kind === "listening";
  const [wasRecording, setWasRecording] = useState(recording);
  if (recording !== wasRecording) {
    setWasRecording(recording);
    if (recording) setBars(SILENT_BARS);
  }
  useEffect(() => {
    const stop = events.overlayFrame.listen((event) => {
      setBars((previous) => nextBars(previous, event.payload.level ?? 0));
    });
    return () => {
      void stop.then((unlisten) => {
        unlisten();
      });
    };
  }, []);
  return bars.length === BAR_COUNT ? bars : SILENT_BARS;
}

/**
 * Animates the pill's width to its content's and reports the shape's size to the backend, which
 * clips the window to it (rule 11). While transcribing the shape is the three dots. The region
 * grows at once and shrinks after the animation.
 */
function usePillShape(
  shape: RefObject<HTMLDivElement | null>,
  content: RefObject<HTMLDivElement | null>,
  dots: boolean,
) {
  const [width, setWidth] = useState<number | null>(null);

  useLayoutEffect(() => {
    const element = content.current;
    if (!element || typeof ResizeObserver === "undefined") return;
    // Rounded up: offsetWidth rounds to the nearest pixel and could clip the last glyph.
    const observer = new ResizeObserver(() => {
      setWidth(Math.ceil(element.getBoundingClientRect().width));
    });
    observer.observe(element);
    return () => {
      observer.disconnect();
    };
  }, [content]);

  const reported = useRef(0);
  useEffect(() => {
    const height = shape.current?.parentElement?.offsetHeight ?? 0;
    if (width === null || height === 0) return;
    const target = dots ? DOTS_SHAPE : { width: width + 2, height };
    const report = () => {
      reported.current = target.width;
      void commands.overlayShape(target.width, target.height);
    };
    if (target.width >= reported.current) {
      report();
      return;
    }
    const timer = setTimeout(report, dots ? DOTS_SETTLED_MS : SHAPE_TRANSITION_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [shape, width, dots]);

  // While transcribing the shape folds into a capsule (styles.css), whatever the content.
  return width === null || dots ? undefined : { width };
}
