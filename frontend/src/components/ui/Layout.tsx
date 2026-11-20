import { type ReactNode, useEffect, useRef } from "react";

export interface PageHeaderProps {
  title: string;
  description?: ReactNode;
  actions?: ReactNode;
  /** Small text above the title, such as a parent page link. */
  eyebrow?: ReactNode;
}

/**
 * The page heading. It takes focus when the page mounts so screen reader and
 * keyboard users land at the start of the new page after navigation.
 */
export function PageHeader({ title, description, actions, eyebrow }: PageHeaderProps) {
  const ref = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    ref.current?.focus({ preventScroll: true });
  }, []);
  return (
    <div className="mb-4 flex flex-wrap items-end justify-between gap-3">
      <div className="flex min-w-0 flex-col gap-1">
        {eyebrow && <div className="text-xs text-fg-muted">{eyebrow}</div>}
        <h1 ref={ref} tabIndex={-1} className="truncate text-xl font-semibold text-fg outline-none">
          {title}
        </h1>
        {description && <div className="text-sm text-fg-muted">{description}</div>}
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </div>
  );
}

export interface CardProps {
  title?: string;
  actions?: ReactNode;
  children: ReactNode;
  /** Remove inner padding, for tables. */
  flush?: boolean;
}

/** A bordered section with an optional heading. */
export function Card({ title, actions, children, flush }: CardProps) {
  return (
    <section
      className="overflow-hidden rounded-lg border border-border bg-surface"
      {...(title ? { "aria-label": title } : {})}
    >
      {(title || actions) && (
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border px-4 py-2.5">
          {title && <h2 className="text-sm font-semibold text-fg">{title}</h2>}
          {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
        </div>
      )}
      <div className={flush ? "" : "p-4"}>{children}</div>
    </section>
  );
}

/** A two-column list of labels and values. */
export function DetailList({ items }: { items: readonly { label: string; value: ReactNode }[] }) {
  return (
    <dl className="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-2 text-sm">
      {items.map((item) => (
        <div key={item.label} className="contents">
          <dt className="text-fg-muted">{item.label}</dt>
          <dd className="min-w-0 break-words text-fg">{item.value}</dd>
        </div>
      ))}
    </dl>
  );
}

/** Inline monospace text for record content, serials, and keys. */
export function Mono({ children }: { children: ReactNode }) {
  return <span className="font-mono text-[13px]">{children}</span>;
}

/** A block of monospace text that wraps long values. */
export function CodeBlock({ children, label }: { children: ReactNode; label?: string }) {
  return (
    <pre
      {...(label ? { "aria-label": label } : {})}
      className="overflow-x-auto rounded-md border border-border bg-surface-muted px-3 py-2 font-mono text-[13px] break-all whitespace-pre-wrap text-fg"
    >
      {children}
    </pre>
  );
}

/** A responsive grid for form fields. */
export function FieldGrid({ children, columns = 2 }: { children: ReactNode; columns?: 1 | 2 | 3 }) {
  const cols = { 1: "grid-cols-1", 2: "grid-cols-1 sm:grid-cols-2", 3: "grid-cols-1 sm:grid-cols-3" }[columns];
  return <div className={`grid gap-3 ${cols}`}>{children}</div>;
}

/** A vertical stack with consistent spacing. */
export function Stack({ children, gap = "md" }: { children: ReactNode; gap?: "sm" | "md" | "lg" }) {
  const g = { sm: "gap-2", md: "gap-4", lg: "gap-6" }[gap];
  return <div className={`flex flex-col ${g}`}>{children}</div>;
}

/** A horizontal row of controls that wraps on small screens. */
export function Toolbar({ children, label }: { children: ReactNode; label?: string }) {
  return (
    <div {...(label ? { role: "group", "aria-label": label } : {})} className="flex flex-wrap items-end gap-2">
      {children}
    </div>
  );
}
