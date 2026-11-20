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

/** A link in the dark sidebar. The current page gets an accent bar and `aria-current`. */
export function NavLink({ href, children, isCurrent, onPress }: NavLinkProps) {
  return (
    <RACLink
      href={href}
      {...(onPress ? { onPress } : {})}
      {...(isCurrent ? { "aria-current": "page" as const } : {})}
      className={[
        "relative flex items-center gap-2.5 rounded-md px-2.5 py-[7px] text-[13px] cursor-pointer transition-colors [&_svg]:size-4",
        "outline-none focus-visible:outline-2 focus-visible:outline-offset-0 focus-visible:outline-sidebar-accent",
        isCurrent
          ? "bg-sidebar-active font-medium text-white before:absolute before:inset-y-1.5 before:-left-3 before:w-0.5 before:rounded-r before:bg-sidebar-accent [&_svg]:text-sidebar-accent"
          : "text-sidebar-muted hovered:bg-sidebar-hover hovered:text-sidebar-fg",
      ].join(" ")}
    >
      {children}
    </RACLink>
  );
}
