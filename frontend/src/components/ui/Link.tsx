import type { ReactNode } from "react";
import { Link as RACLink, type LinkProps as RACLinkProps } from "react-aria-components";
import { type ButtonSize, type ButtonVariant, buttonClasses } from "./Button";
import { focusRing } from "./styles";

export interface LinkProps extends Omit<RACLinkProps, "className" | "style"> {
  /** Render as a button. */
  variant?: "inline" | ButtonVariant;
  size?: ButtonSize;
}

function classesFor(variant: LinkProps["variant"], size: ButtonSize): string {
  if (variant === undefined || variant === "inline") {
    return `rounded-sm text-accent-text underline-offset-2 hovered:underline cursor-pointer ${focusRing}`;
  }
  return buttonClasses(variant, size);
}

/** An in-app link. Navigation goes through the client router. */
export function Link({ variant = "inline", size = "md", ...props }: LinkProps) {
  return <RACLink {...props} className={classesFor(variant, size)} />;
}

export interface DocumentLinkProps {
  href: string;
  children: ReactNode;
  variant?: "inline" | ButtonVariant;
  size?: ButtonSize;
}

/** A link that loads a new document, bypassing the client router. Used for server redirects such as single sign-on. */
export function DocumentLink({ href, children, variant = "inline", size = "md" }: DocumentLinkProps) {
  return (
    <a href={href} className={classesFor(variant, size)}>
      {children}
    </a>
  );
}

export interface NavLinkProps {
  href: string;
  children: ReactNode;
  isCurrent: boolean;
  onPress?: () => void;
}

/** A sidebar navigation link. The current page uses the accent color and `aria-current`. */
export function NavLink({ href, children, isCurrent, onPress }: NavLinkProps) {
  return (
    <RACLink
      href={href}
      {...(onPress ? { onPress } : {})}
      {...(isCurrent ? { "aria-current": "page" as const } : {})}
      className={[
        "flex items-center gap-2.5 rounded-md px-2.5 py-1.5 text-sm cursor-pointer [&_svg]:size-4",
        focusRing,
        isCurrent
          ? "bg-accent-soft font-medium text-accent-text"
          : "text-fg-muted hovered:bg-surface-hover hovered:text-fg",
      ].join(" ")}
    >
      {children}
    </RACLink>
  );
}
