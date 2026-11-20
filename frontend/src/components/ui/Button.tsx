import { Button as RACButton, type ButtonProps as RACButtonProps } from "react-aria-components";
import { focusRing } from "./styles";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md" | "icon";

export interface ButtonProps extends Omit<RACButtonProps, "className" | "style"> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

const variants: Record<ButtonVariant, string> = {
  primary:
    "bg-accent text-accent-fg hovered:bg-accent-hover border border-transparent shadow-[0_1px_2px_rgb(15_23_42/0.15)]",
  secondary:
    "bg-surface text-fg border border-border-strong hovered:bg-surface-muted shadow-[0_1px_2px_rgb(15_23_42/0.05)]",
  ghost: "bg-transparent text-fg border border-transparent hovered:bg-surface-hover",
  danger: "bg-danger text-danger-fg hovered:bg-danger-hover border border-transparent",
};

const sizes: Record<ButtonSize, string> = {
  sm: "h-7 px-2 text-xs gap-1",
  md: "h-8 px-3 text-sm gap-1.5",
  icon: "h-7 w-7 justify-center p-0",
};

/** Builds button classes. Used by link components that look like buttons. */
export function buttonClasses(variant: ButtonVariant = "secondary", size: ButtonSize = "md"): string {
  return [
    "inline-flex shrink-0 cursor-default items-center rounded-md font-medium whitespace-nowrap transition-colors",
    "disabled:cursor-not-allowed disabled:opacity-50 pending:opacity-70 [&_svg]:size-4",
    focusRing,
    variants[variant],
    sizes[size],
  ].join(" ");
}

/** A button. Use `variant="primary"` for the main action of a view. */
export function Button({ variant = "secondary", size = "md", ...props }: ButtonProps) {
  return <RACButton {...props} className={buttonClasses(variant, size)} />;
}
