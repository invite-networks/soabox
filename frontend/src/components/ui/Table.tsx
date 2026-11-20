import type { ReactNode } from "react";

/*
 * Tables are static HTML tables. They hold buttons and links, which keep their
 * normal tab order, so no grid keyboard model is needed.
 */

export interface TableProps {
  /** Accessible name for the table. */
  label: string;
  children: ReactNode;
}

export function Table({ label, children }: TableProps) {
  return (
    <div className="overflow-x-auto">
      <table aria-label={label} className="w-full border-collapse text-sm">
        {children}
      </table>
    </div>
  );
}

export function THead({ children }: { children: ReactNode }) {
  return <thead className="border-b border-border bg-surface-muted text-left text-xs text-fg-muted">{children}</thead>;
}

export function TBody({ children }: { children: ReactNode }) {
  return <tbody className="divide-y divide-border">{children}</tbody>;
}

export function Tr({ children, muted }: { children: ReactNode; muted?: boolean }) {
  return <tr className={`hover:bg-surface-muted ${muted ? "text-fg-muted" : ""}`}>{children}</tr>;
}

export interface ThProps {
  children?: ReactNode;
  align?: "left" | "right";
  /** Visually hidden header, for example for an actions column. */
  hidden?: boolean;
}

export function Th({ children, align = "left", hidden }: ThProps) {
  return (
    <th scope="col" className={`px-3 py-2 font-medium whitespace-nowrap ${align === "right" ? "text-right" : ""}`}>
      {hidden ? <span className="sr-only">{children}</span> : children}
    </th>
  );
}

export interface TdProps {
  children?: ReactNode;
  align?: "left" | "right";
  mono?: boolean;
  wrap?: boolean;
  colSpan?: number;
}

export function Td({ children, align = "left", mono, wrap, colSpan }: TdProps) {
  return (
    <td
      {...(colSpan ? { colSpan } : {})}
      className={[
        "px-3 py-1.5 align-top",
        align === "right" ? "text-right" : "",
        mono ? "font-mono text-[13px]" : "",
        wrap ? "break-all" : "whitespace-nowrap",
      ].join(" ")}
    >
      {children}
    </td>
  );
}

/** A row header cell. */
export function RowHeader({ children, mono }: { children: ReactNode; mono?: boolean }) {
  return (
    <th
      scope="row"
      className={`px-3 py-1.5 text-left align-top font-normal whitespace-nowrap ${mono ? "font-mono text-[13px]" : ""}`}
    >
      {children}
    </th>
  );
}

/** Buttons in a table row, aligned right. */
export function RowActions({ children }: { children: ReactNode }) {
  return <div className="flex items-center justify-end gap-1">{children}</div>;
}
