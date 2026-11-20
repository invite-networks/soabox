import { detailText, parseDiff } from "../lib/diff";
import { CodeBlock } from "./ui";

const lineClasses = {
  added: "bg-success-soft text-fg",
  removed: "bg-danger-soft text-fg",
  unchanged: "text-fg-muted",
} as const;

const markers = {
  added: { symbol: "+", label: "Added" },
  removed: { symbol: "-", label: "Removed" },
  unchanged: { symbol: " ", label: "Unchanged" },
} as const;

/** Renders a history diff as before and after lines, followed by any request detail. */
export function DiffView({ diff, detail }: { diff: unknown; detail?: unknown }) {
  const entries = parseDiff(diff);
  const { error, request } = detailText(detail);
  return (
    <div className="flex flex-col gap-3">
      {error && <p className="text-sm text-danger">{error}</p>}
      {entries.length === 0 && !request && !error && (
        <p className="text-sm text-fg-muted">No changes were stored for this entry.</p>
      )}
      {entries.map((entry, index) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: entries are a fixed snapshot that never reorders.
        <div key={`${entry.title}-${index}`} className="overflow-hidden rounded-md border border-border">
          <div className="border-b border-border bg-surface-muted px-3 py-1.5 font-mono text-xs font-medium text-fg">
            {entry.title}
          </div>
          <ul className="font-mono text-[13px]">
            {entry.lines.length === 0 && <li className="px-3 py-1 text-fg-muted">No values</li>}
            {entry.lines.map((line, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: lines are a fixed snapshot that never reorders.
              <li key={`${line.kind}-${i}`} className={`flex gap-2 px-3 py-0.5 break-all ${lineClasses[line.kind]}`}>
                <span aria-hidden className="w-3 shrink-0 select-none">
                  {markers[line.kind].symbol}
                </span>
                <span className="sr-only">{markers[line.kind].label}: </span>
                <span className="whitespace-pre-wrap">{line.text}</span>
              </li>
            ))}
          </ul>
        </div>
      ))}
      {request && <CodeBlock label="Request">{request}</CodeBlock>}
    </div>
  );
}
