import type { ReactNode } from "react";
import {
  Header,
  Popover,
  Menu as RACMenu,
  MenuItem as RACMenuItem,
  MenuSection as RACMenuSection,
  MenuTrigger as RACMenuTrigger,
  Separator,
} from "react-aria-components";
import { listItem, popover } from "./styles";

export function MenuTrigger({ children }: { children: ReactNode }) {
  return <RACMenuTrigger>{children}</RACMenuTrigger>;
}

export interface MenuProps {
  label: string;
  children: ReactNode;
  onAction?: (key: string) => void;
  placement?: "bottom start" | "bottom end";
}

/** A popup menu. Place it after the trigger button inside {@link MenuTrigger}. */
export function Menu({ label, children, onAction, placement = "bottom end" }: MenuProps) {
  return (
    <Popover placement={placement} className={`${popover} min-w-48`}>
      <RACMenu
        aria-label={label}
        {...(onAction ? { onAction: (key) => onAction(String(key)) } : {})}
        className="outline-none"
      >
        {children}
      </RACMenu>
    </Popover>
  );
}

export interface MenuItemProps {
  id: string;
  children: ReactNode;
  textValue?: string;
  isDanger?: boolean;
  href?: string;
}

export function MenuItem({ id, children, textValue, isDanger, href }: MenuItemProps) {
  return (
    <RACMenuItem
      id={id}
      {...(textValue ? { textValue } : {})}
      {...(href ? { href } : {})}
      className={`${listItem} [&_svg]:size-4 ${isDanger ? "text-danger" : ""}`}
    >
      {children}
    </RACMenuItem>
  );
}

export function MenuSection({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <RACMenuSection>
      {title && <Header className="px-2 pt-1.5 pb-1 text-xs font-medium text-fg-muted">{title}</Header>}
      {children}
    </RACMenuSection>
  );
}

export function MenuSeparator() {
  return <Separator className="my-1 border-t border-border" />;
}
