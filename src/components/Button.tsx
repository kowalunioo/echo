import type { ButtonHTMLAttributes, Ref } from "react";

export type ButtonVariant = "contrast" | "secondary" | "quiet" | "danger";
export type ButtonSize = "default" | "compact";

const VARIANTS: Record<ButtonVariant, string> = {
  // Ink on paper: the strong action. Lavender stays for state marks, never for a button.
  contrast: "bg-fg text-bg hover:opacity-90",
  secondary: "border border-control bg-surface hover:bg-raised",
  quiet: "text-muted hover:bg-raised hover:text-fg",
  danger: "bg-danger text-danger-fg hover:opacity-90",
};

/**
 * Two sizes only: `default` for the main action of a page, step or panel, `compact` for actions
 * inside a card, a row or a dialog. Both use the body text size, so no action reads as an
 * afterthought.
 */
const SIZES: Record<ButtonSize, string> = {
  default: "px-4 py-2",
  compact: "px-3.5 py-1.5",
};

/** The one filled or outlined button of the main window. Text links stay plain buttons. */
export function Button({
  variant = "contrast",
  size = "compact",
  type = "button",
  className = "",
  ref,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: ButtonSize;
  ref?: Ref<HTMLButtonElement>;
}) {
  return (
    <button
      ref={ref}
      type={type}
      className={`shrink-0 rounded-lg text-sm font-medium whitespace-nowrap transition-[opacity,background-color,color,transform] duration-150 ease-out-strong active:scale-[0.97] motion-reduce:active:scale-100 disabled:cursor-not-allowed disabled:opacity-50 disabled:active:scale-100 ${SIZES[size]} ${VARIANTS[variant]} ${className}`}
      {...props}
    />
  );
}
