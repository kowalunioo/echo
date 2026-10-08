import { type KeyboardEvent, type ReactNode, useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import type { ModelEntry, ModelId } from "../bindings";
import { Button } from "../components/Button";
import { CheckIcon, ChevronIcon } from "../components/icons";
import { useModels } from "../store/models";
import { useShell } from "../store/shell";
import { downloadingEntry, percentOf } from "./view";

type Tone = "ready" | "busy" | "idle" | "error";

/**
 * The compact Model indicator in the sidebar: the active Model and its state (models.md "UI",
 * settings-and-first-run.md status area). With Models on this computer it is a drop-down that
 * switches between them, with "Download another Model…" leading to the Model page; with none it
 * opens that page, which says to download one.
 */
export function ModelIndicator() {
  const { t } = useTranslation();
  const models = useModels((s) => s.state);
  const load = useModels((s) => s.load);
  const activate = useModels((s) => s.activate);
  const setPage = useShell((s) => s.setPage);

  useEffect(() => {
    void load();
  }, [load]);

  if (!models) return null;
  const active = models.models.find((m) => m.id === models.active);
  const loadingTarget = models.models.find((m) => m.id === models.activating);
  const downloading = downloadingEntry(models);
  const onDisk = models.models.filter((m) => m.downloaded);

  let name: string;
  let detail: string;
  let tone: Tone;
  if (loadingTarget) {
    name = loadingTarget.name;
    detail = t("models.indicator.loading");
    tone = "busy";
  } else if (active) {
    name = active.name;
    const byState = {
      ready: ["ready", "ready"],
      loading: ["loading", "busy"],
      unloaded: ["unloaded", "idle"],
      error: ["error", "error"],
      none: ["ready", "ready"],
    } as const;
    const [key, stateTone] = byState[models.activeState];
    detail = t(`models.indicator.${key}`);
    tone = stateTone;
  } else if (downloading) {
    name = downloading.name;
    const progress =
      downloading.download.state === "downloading"
        ? percentOf(downloading.download.downloaded, downloading.download.total)
        : 100;
    detail = t("models.indicator.downloading", { percent: progress });
    tone = "busy";
  } else {
    name = t("models.indicator.none");
    detail = "";
    tone = "idle";
  }

  // Ready is a hollow ring: solid lavender belongs to Listening, and the label says "Ready".
  const dot = {
    ready: "border-[1.5px] border-muted",
    busy: "bg-accent animate-pulse",
    idle: "bg-muted/50",
    error: "bg-danger",
  }[tone];
  const row = (
    <>
      <span className={`size-2 shrink-0 rounded-full ${dot}`} aria-hidden="true" />
      <span title={name} className="min-w-0 flex-1 truncate">
        {name}
      </span>
      {detail && (
        // One line, always: a long state ("Freed from memory…") shares it with the name and
        // both are cut with an ellipsis; the full text is in the title and the accessible name.
        <span
          title={detail}
          className={`max-w-[55%] truncate text-xs ${tone === "error" ? "text-danger" : "text-muted"}`}
        >
          {detail}
        </span>
      )}
    </>
  );
  const rowClass =
    "flex w-full items-center gap-2.5 rounded-[9px] px-2.5 py-1.5 text-left text-note transition-colors duration-150 hover:bg-surface";
  const described = `${name}${detail ? `, ${detail}` : ""}`;

  if (onDisk.length === 0) {
    return (
      <button
        type="button"
        onClick={() => {
          setPage("model");
        }}
        aria-label={`${t("models.indicator.open")}: ${described}`}
        className={rowClass}
      >
        {row}
      </button>
    );
  }
  return (
    <ModelMenu
      label={`${t("models.indicator.label")}: ${described}`}
      options={onDisk}
      active={models.active}
      busy={models.activating !== null}
      className={rowClass}
      onChoose={(id) => void activate(id)}
      onMore={() => {
        setPage("model");
      }}
    >
      {row}
      <span className="-ml-1 flex" aria-hidden="true">
        <ChevronIcon />
      </span>
    </ModelMenu>
  );
}

/**
 * The drop-down of the Models on this computer, opening upwards from the sidebar's foot. The
 * same keyboard contract as the Microphone picker: arrows move, Enter or Space choose, Escape
 * closes, a click elsewhere closes.
 */
function ModelMenu({
  label,
  options,
  active,
  busy,
  className,
  onChoose,
  onMore,
  children,
}: {
  label: string;
  options: ModelEntry[];
  active: ModelId | null;
  busy: boolean;
  className: string;
  onChoose: (id: ModelId) => void;
  onMore: () => void;
  children: ReactNode;
}) {
  const { t } = useTranslation();
  const id = useId();
  const [open, setOpen] = useState(false);
  const [highlighted, setHighlighted] = useState(0);
  const button = useRef<HTMLButtonElement>(null);
  const listbox = useRef<HTMLUListElement>(null);
  const root = useRef<HTMLDivElement>(null);
  // The last entry is "Download another Model…".
  const count = options.length + 1;
  const activeIndex = Math.max(
    0,
    options.findIndex((m) => m.id === active),
  );

  const show = () => {
    setHighlighted(activeIndex);
    setOpen(true);
  };
  const close = (refocus: boolean) => {
    setOpen(false);
    if (refocus) button.current?.focus();
  };
  const choose = (index: number) => {
    const option = options[index];
    if (option === undefined) onMore();
    else if (option.id !== active && !busy) onChoose(option.id);
    close(true);
  };

  useEffect(() => {
    if (open) listbox.current?.focus();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
    };
  }, [open]);

  const onButtonKey = (event: KeyboardEvent) => {
    if (["ArrowDown", "ArrowUp"].includes(event.key)) {
      event.preventDefault();
      show();
    }
  };
  const onListKey = (event: KeyboardEvent) => {
    const last = count - 1;
    const keys: Record<string, () => void> = {
      ArrowDown: () => {
        setHighlighted((i) => Math.min(last, i + 1));
      },
      ArrowUp: () => {
        setHighlighted((i) => Math.max(0, i - 1));
      },
      Home: () => {
        setHighlighted(0);
      },
      End: () => {
        setHighlighted(last);
      },
      Enter: () => {
        choose(highlighted);
      },
      " ": () => {
        choose(highlighted);
      },
      Escape: () => {
        close(true);
      },
    };
    if (event.key === "Tab") {
      close(false);
      return;
    }
    const action = keys[event.key];
    if (action) {
      event.preventDefault();
      action();
    }
  };
  const optionClass = (index: number) =>
    `flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-note ${
      index === highlighted ? "bg-raised" : ""
    }`;

  return (
    <div ref={root} className="relative">
      <button
        ref={button}
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? `${id}-list` : undefined}
        aria-label={label}
        onClick={() => {
          if (open) close(false);
          else show();
        }}
        onKeyDown={onButtonKey}
        className={className}
      >
        {children}
      </button>
      {open && (
        <ul
          ref={listbox}
          id={`${id}-list`}
          role="listbox"
          tabIndex={-1}
          aria-label={t("models.indicator.label")}
          aria-activedescendant={`${id}-option-${highlighted}`}
          onKeyDown={onListKey}
          className="absolute bottom-full left-0 z-10 mb-1.5 flex w-full flex-col rounded-xl border border-line bg-surface p-1 shadow-lg transition-[opacity,transform] duration-150 ease-out-strong origin-bottom-left starting:scale-[0.97] starting:opacity-0 motion-reduce:starting:scale-100"
        >
          {options.map((option, index) => {
            const isActive = option.id === active;
            return (
              <li
                key={option.id}
                id={`${id}-option-${index}`}
                role="option"
                aria-selected={isActive}
                onPointerMove={() => {
                  setHighlighted(index);
                }}
                onClick={() => {
                  choose(index);
                }}
                className={`${optionClass(index)} ${isActive ? "font-medium" : ""}`}
              >
                <span title={option.name} className="min-w-0 flex-1 truncate">
                  {option.name}
                </span>
                {isActive && <CheckIcon />}
              </li>
            );
          })}
          <li role="separator" className="mx-2 my-1 border-t border-line" />
          <li
            id={`${id}-option-${options.length}`}
            role="option"
            aria-selected={false}
            onPointerMove={() => {
              setHighlighted(options.length);
            }}
            onClick={() => {
              choose(options.length);
            }}
            className={`${optionClass(options.length)} text-muted`}
          >
            {t("models.indicator.more")}
          </li>
        </ul>
      )}
    </div>
  );
}

/**
 * The prominent "Download a Model to start" panel shown after onboarding when no Model is active
 * (settings-and-first-run.md rule 5).
 */
export function NoModelPanel({ onModelsPage }: { onModelsPage: boolean }) {
  const { t } = useTranslation();
  const models = useModels((s) => s.state);
  const setPage = useShell((s) => s.setPage);

  if (!models || models.active !== null || models.activating !== null) return null;
  return (
    <section className="flex items-center justify-between gap-6 rounded-card border border-accent/40 bg-accent-soft px-6 py-5">
      <div className="flex flex-col gap-1">
        <h2 className="text-heading text-accent-soft-fg">{t("models.start.title")}</h2>
        <p className="text-sm text-accent-soft-fg">{t("models.start.body")}</p>
      </div>
      {!onModelsPage && (
        <Button
          size="default"
          onClick={() => {
            setPage("model");
          }}
        >
          {t("models.start.action")}
        </Button>
      )}
    </section>
  );
}
