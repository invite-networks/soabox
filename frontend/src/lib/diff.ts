/**
 * Turns a history diff into readable before and after lines.
 *
 * The contract (docs/spec/v1-api.md, History) defines two shapes:
 * - record changes: `[{name, type, before: {ttl, records} | null, after: same | null}]`
 * - zone setting changes: `{zone: [{field, before, after}]}`
 * Anything else is shown as JSON so nothing is hidden.
 */

import type { RecordChange, RrSetSnapshot, ZoneFieldChange } from "../api/types";

export interface DiffLine {
  kind: "added" | "removed" | "unchanged";
  text: string;
}

export interface DiffEntry {
  title: string;
  lines: DiffLine[];
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isSnapshot(value: unknown): value is RrSetSnapshot | null {
  return value === null || (isObject(value) && typeof value["ttl"] === "number" && Array.isArray(value["records"]));
}

function isRecordChange(value: unknown): value is RecordChange {
  return (
    isObject(value) &&
    typeof value["name"] === "string" &&
    typeof value["type"] === "string" &&
    isSnapshot(value["before"] ?? null) &&
    isSnapshot(value["after"] ?? null)
  );
}

function isZoneFieldChange(value: unknown): value is ZoneFieldChange {
  return isObject(value) && typeof value["field"] === "string";
}

/** Formats any value for a diff line. */
export function stringify(value: unknown): string {
  if (value === null || value === undefined) {
    return "none";
  }
  if (typeof value === "string") {
    return value;
  }
  return JSON.stringify(value);
}

/** Lines for one side of a record change: the TTL, then each value. */
export function snapshotLines(snapshot: RrSetSnapshot | null | undefined): string[] {
  if (!snapshot) {
    return [];
  }
  return [`TTL ${snapshot.ttl}`, ...snapshot.records.map((r) => (r.disabled ? `${r.content} (disabled)` : r.content))];
}

/** Marks lines as removed, unchanged, or added. */
export function compareLines(before: readonly string[], after: readonly string[]): DiffLine[] {
  const lines: DiffLine[] = before.map((text) => ({ kind: after.includes(text) ? "unchanged" : "removed", text }));
  for (const text of after) {
    if (!before.includes(text)) {
      lines.push({ kind: "added", text });
    }
  }
  return lines;
}

/** Entries for a list of record changes. */
export function recordEntries(changes: readonly RecordChange[]): DiffEntry[] {
  return changes.map((c) => ({
    title: `${c.name} ${c.type}`,
    lines: compareLines(snapshotLines(c.before), snapshotLines(c.after)),
  }));
}

function fieldEntry(change: ZoneFieldChange): DiffEntry {
  const before = stringify(change.before);
  const after = stringify(change.after);
  return {
    title: change.field,
    lines: before === after ? [{ kind: "unchanged", text: after }] : compareLines([before], [after]),
  };
}

function fallback(diff: unknown): DiffEntry[] {
  return [
    {
      title: "Change",
      lines: JSON.stringify(diff, null, 2)
        .split("\n")
        .map((text) => ({ kind: "unchanged", text })),
    },
  ];
}

/** Parses a history diff. Returns no entries for an empty diff. */
export function parseDiff(diff: unknown): DiffEntry[] {
  if (diff === null || diff === undefined) {
    return [];
  }
  if (Array.isArray(diff)) {
    return diff.every(isRecordChange) ? recordEntries(diff) : fallback(diff);
  }
  if (isObject(diff) && Array.isArray(diff["zone"]) && diff["zone"].every(isZoneFieldChange)) {
    return diff["zone"].map(fieldEntry);
  }
  return fallback(diff);
}

/**
 * Text for a history entry's `detail`. Failures carry `{request, error}`, and
 * the error is what the reader needs. Other details are the redacted request.
 */
export function detailText(detail: unknown): { error: string | null; request: string | null } {
  if (detail === null || detail === undefined) {
    return { error: null, request: null };
  }
  if (typeof detail === "string") {
    return { error: null, request: detail };
  }
  if (isObject(detail) && "error" in detail) {
    const request = detail["request"];
    return {
      error: stringify(detail["error"]),
      request: request === null || request === undefined ? null : JSON.stringify(request, null, 2),
    };
  }
  return { error: null, request: JSON.stringify(detail, null, 2) };
}
