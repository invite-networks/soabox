/** Shared class names for the component layer. Theme colors come from src/styles/theme.css. */

export const focusRing =
  "outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus";

export const fieldInput = [
  "w-full rounded-md border border-border-strong bg-surface px-2.5 py-1.5 text-sm text-fg",
  "placeholder:text-fg-muted outline-none",
  "focus:border-focus focus:outline-2 focus:outline-offset-0 focus:outline-focus/40",
  "invalid:border-danger disabled:cursor-not-allowed disabled:opacity-60",
].join(" ");

export const fieldLabel = "text-sm font-medium text-fg";
export const fieldDescription = "text-xs text-fg-muted";
export const fieldError = "text-xs font-medium text-danger";

export const popover =
  "min-w-(--trigger-width) overflow-auto rounded-md border border-border bg-surface p-1 shadow-lg outline-none max-h-72";

export const listItem = [
  "flex cursor-default items-center gap-2 rounded px-2 py-1.5 text-sm text-fg outline-none",
  "hovered:bg-surface-hover focused:bg-surface-hover selected:font-medium disabled:opacity-50",
].join(" ");
