import { type KeyboardEvent, useCallback, useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { type DeviceList, type MicrophoneChoice, commands } from "../bindings";
import { useSetting, useSettings } from "../store/settings";
import { Row } from "./AppSettings";

type ListState =
  | { status: "loading"; list: DeviceList | null }
  | { status: "ready"; list: DeviceList }
  | { status: "error"; list: DeviceList | null };

interface Option {
  choice: MicrophoneChoice;
  label: string;
}

const DEFAULT_CHOICE: MicrophoneChoice = { kind: "default" };

/** A fresh device list from the backend, or `null` if it could not be loaded. */
async function loadDeviceList(): Promise<DeviceList | null> {
  try {
    const result = await commands.listMicrophones();
    if (result.status === "ok") return result.data;
    console.error("list_microphones failed", result.error);
  } catch (error) {
    console.error("list_microphones failed", error);
  }
  return null;
}

function sameChoice(a: MicrophoneChoice, b: MicrophoneChoice) {
  return a.kind === b.kind && (a.kind === "default" || (b.kind === "device" && a.name === b.name));
}

/**
 * The Microphone section of the Dictation page (microphone.md "UI"): a picker with
 * "Default (<Windows default>)" first and then every input device, a "(not connected)" mark for
 * a stored device that is missing, and a reset to Default. The device list is fetched afresh
 * every time the picker opens (rule 8).
 */
export function MicrophoneSettings() {
  const { t } = useTranslation();
  const [choice, setChoice] = useSetting("microphone");
  const [state, setState] = useState<ListState>({ status: "loading", list: null });

  const applyAnswer = useCallback((answer: DeviceList | null) => {
    setState((s) =>
      answer ? { status: "ready", list: answer } : { status: "error", list: s.list },
    );
  }, []);

  // The closed picker shows the Windows default's name, so fetch once when the page opens.
  useEffect(() => {
    void loadDeviceList().then(applyAnswer);
  }, [applyAnswer]);

  const refresh = () => {
    setState((s) => ({ status: "loading", list: s.list }));
    void loadDeviceList().then(applyAnswer);
  };

  const list = state.list;
  const defaultLabel = list?.default
    ? t("settings.microphone.defaultOption", { name: list.default })
    : t("settings.microphone.defaultUnknown");
  const missing = choice.kind === "device" && list !== null && !list.devices.includes(choice.name);
  const options: Option[] = [
    { choice: DEFAULT_CHOICE, label: defaultLabel },
    ...(list?.devices ?? []).map((name) => ({
      choice: { kind: "device", name } as const,
      label: name,
    })),
  ];
  if (missing) {
    options.push({
      choice,
      label: t("settings.microphone.notConnected", { name: choice.name }),
    });
  }
  const selectedLabel =
    choice.kind === "default"
      ? defaultLabel
      : missing
        ? t("settings.microphone.notConnected", { name: choice.name })
        : choice.name;

  return (
    <section className="divide-y divide-line rounded-card border border-line bg-surface">
      <Row
        label={t("settings.microphone.label")}
        description={t("settings.microphone.description")}
      >
        <div className="flex flex-col items-end gap-1.5">
          <Picker
            label={t("settings.microphone.label")}
            selectedLabel={selectedLabel}
            missing={missing}
            options={options}
            selected={choice}
            loading={state.status === "loading"}
            failed={state.status === "error"}
            onOpen={refresh}
            onSelect={(next) => void setChoice(next)}
          />
          {choice.kind === "device" && (
            <button
              type="button"
              onClick={() => void useSettings.getState().reset("microphone")}
              className="rounded-md text-xs font-medium text-accent-strong underline-offset-4 hover:underline"
            >
              {t("settings.microphone.reset")}
            </button>
          )}
        </div>
      </Row>
      {state.status === "ready" && state.list.devices.length === 0 && (
        <p role="status" className="px-6 py-3 text-sm text-muted">
          {t("settings.microphone.none")}
        </p>
      )}
    </section>
  );
}

function Picker({
  label,
  selectedLabel,
  missing,
  options,
  selected,
  loading,
  failed,
  onOpen,
  onSelect,
}: {
  label: string;
  selectedLabel: string;
  missing: boolean;
  options: Option[];
  selected: MicrophoneChoice;
  loading: boolean;
  failed: boolean;
  onOpen: () => void;
  onSelect: (choice: MicrophoneChoice) => void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const button = useRef<HTMLButtonElement>(null);
  const listbox = useRef<HTMLUListElement>(null);
  const root = useRef<HTMLDivElement>(null);
  const selectedIndex = Math.max(
    0,
    options.findIndex((o) => sameChoice(o.choice, selected)),
  );

  const show = () => {
    onOpen();
    setActive(selectedIndex);
    setOpen(true);
  };
  const close = (refocus: boolean) => {
    setOpen(false);
    if (refocus) button.current?.focus();
  };
  const choose = (index: number) => {
    const option = options[index];
    if (option) onSelect(option.choice);
    close(true);
  };

  useEffect(() => {
    if (open) listbox.current?.focus();
  }, [open]);

  // Clicking anywhere else closes the list.
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
    const last = options.length - 1;
    const keys: Record<string, () => void> = {
      ArrowDown: () => {
        setActive((i) => Math.min(last, i + 1));
      },
      ArrowUp: () => {
        setActive((i) => Math.max(0, i - 1));
      },
      Home: () => {
        setActive(0);
      },
      End: () => {
        setActive(last);
      },
      Enter: () => {
        choose(active);
      },
      " ": () => {
        choose(active);
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

  return (
    <div ref={root} className="relative">
      <button
        ref={button}
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? `${id}-list` : undefined}
        aria-label={`${label}: ${selectedLabel}`}
        onClick={() => {
          if (open) close(false);
          else show();
        }}
        onKeyDown={onButtonKey}
        className="flex w-72 items-center justify-between gap-3 rounded-lg border border-line bg-bg px-3 py-1.5 text-left text-sm transition-colors duration-150 hover:border-muted/50 focus-visible:outline-2 focus-visible:outline-focus"
      >
        <span className={`truncate ${missing ? "text-muted" : ""}`}>{selectedLabel}</span>
        <Chevron />
      </button>
      {open && (
        <ul
          ref={listbox}
          id={`${id}-list`}
          role="listbox"
          tabIndex={-1}
          aria-label={label}
          aria-busy={loading}
          aria-activedescendant={`${id}-option-${active}`}
          onKeyDown={onListKey}
          className="absolute right-0 z-10 mt-1.5 flex max-h-72 w-80 flex-col overflow-y-auto rounded-xl border border-line bg-surface p-1 shadow-lg outline-none"
        >
          {options.map((option, index) => {
            const isSelected = sameChoice(option.choice, selected);
            return (
              <li
                key={option.choice.kind === "default" ? "default" : `device:${option.choice.name}`}
                id={`${id}-option-${index}`}
                role="option"
                aria-selected={isSelected}
                onPointerMove={() => {
                  setActive(index);
                }}
                onClick={() => {
                  choose(index);
                }}
                className={`flex cursor-pointer items-center justify-between gap-3 rounded-lg px-3 py-2 text-sm ${
                  index === active ? "bg-raised" : ""
                } ${isSelected ? "font-medium" : ""}`}
              >
                <span className="truncate">{option.label}</span>
                {isSelected && <Check />}
              </li>
            );
          })}
          {loading && (
            <li role="presentation" className="px-3 py-2 text-xs text-muted" aria-live="polite">
              {t("settings.microphone.loading")}
            </li>
          )}
          {failed && (
            <li role="presentation" className="px-3 py-2 text-xs text-muted">
              {t("settings.microphone.listFailed")}
            </li>
          )}
        </ul>
      )}
    </div>
  );
}

function Chevron() {
  return (
    <svg
      viewBox="0 0 24 24"
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className="shrink-0 text-muted"
    >
      <path d="m6 9 6 6 6-6" />
    </svg>
  );
}

function Check() {
  return (
    <svg
      viewBox="0 0 24 24"
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className="shrink-0 text-accent"
    >
      <path d="m5 12 5 5 9-10" />
    </svg>
  );
}
