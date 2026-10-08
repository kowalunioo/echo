/**
 * An on/off switch. On is the filled accent track; off is an outlined track (the edge uses
 * --echo-control, so the off switch stays visible against every surface).
 */
export function Switch({
  checked,
  label,
  disabled = false,
  onChange,
}: {
  checked: boolean;
  /** The accessible name; normally the label of the row the switch sits in. */
  label: string;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => {
        onChange(!checked);
      }}
      className={`relative inline-flex h-6 w-11 items-center rounded-full transition-[background-color,border-color,transform] duration-150 ease-out-strong active:scale-[0.97] disabled:cursor-not-allowed disabled:opacity-50 ${
        checked ? "bg-accent-strong" : "border border-control bg-raised"
      }`}
    >
      <span
        aria-hidden
        className={`inline-block size-4 rounded-full shadow transition-transform duration-150 ease-out-strong ${
          checked ? "translate-x-6 bg-accent-fg" : "translate-x-1 bg-muted"
        }`}
      />
    </button>
  );
}
