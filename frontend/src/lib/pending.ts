/**
 * Pending record changes for the zone editor.
 *
 * Changes are kept per RRset (name and type). Each holds the RRset as loaded
 * from the server (`before`) and the RRset the user wants (`after`). A `null`
 * `after` deletes the RRset. Everything is sent in one PATCH.
 */

import type { PatchRrSetsRequest, RrSet } from "../api/types";

export interface PendingChange {
  key: string;
  name: string;
  type: string;
  before: RrSet | null;
  after: RrSet | null;
}

export type PendingChanges = ReadonlyMap<string, PendingChange>;

export type ChangeStatus = "added" | "changed" | "deleted";

/** Key for an RRset. Names compare case-insensitively. */
export function rrKey(name: string, type: string): string {
  return `${name.toLowerCase()}|${type.toUpperCase()}`;
}

/** Returns the status shown for a pending change. */
export function changeStatus(change: PendingChange): ChangeStatus {
  if (change.after === null) {
    return "deleted";
  }
  return change.before === null ? "added" : "changed";
}

function sameRecords(a: RrSet, b: RrSet): boolean {
  if (a.ttl !== b.ttl || a.records.length !== b.records.length) {
    return false;
  }
  const left = a.records.map((r) => `${r.content}|${r.disabled}`).sort();
  const right = b.records.map((r) => `${r.content}|${r.disabled}`).sort();
  return left.every((value, i) => value === right[i]);
}

/**
 * Records a change. `before` is the server version, or `null` for a new RRset.
 * A change that returns an RRset to its server state is dropped.
 */
export function withChange(
  changes: PendingChanges,
  before: RrSet | null,
  after: RrSet | null,
): Map<string, PendingChange> {
  const source = after ?? before;
  if (!source) {
    return new Map(changes);
  }
  const key = rrKey(source.name, source.type);
  const next = new Map(changes);
  if ((before === null && after === null) || (before !== null && after !== null && sameRecords(before, after))) {
    next.delete(key);
    return next;
  }
  next.set(key, { key, name: source.name, type: source.type, before, after });
  return next;
}

/** Removes a change. */
export function withoutChange(changes: PendingChanges, key: string): Map<string, PendingChange> {
  const next = new Map(changes);
  next.delete(key);
  return next;
}

const TYPE_ORDER = ["SOA", "NS"];

function compareNames(a: string, b: string): number {
  const la = a.toLowerCase().split(".").reverse();
  const lb = b.toLowerCase().split(".").reverse();
  for (let i = 0; i < Math.max(la.length, lb.length); i++) {
    const x = la[i];
    const y = lb[i];
    if (x === undefined) {
      return -1;
    }
    if (y === undefined) {
      return 1;
    }
    if (x !== y) {
      return x < y ? -1 : 1;
    }
  }
  return 0;
}

/** Sorts RRsets by name from the apex down, then SOA and NS first, then type. */
export function compareRrSets(a: { name: string; type: string }, b: { name: string; type: string }): number {
  const byName = compareNames(a.name, b.name);
  if (byName !== 0) {
    return byName;
  }
  const ia = TYPE_ORDER.indexOf(a.type);
  const ib = TYPE_ORDER.indexOf(b.type);
  if (ia !== ib) {
    return (ia === -1 ? 99 : ia) - (ib === -1 ? 99 : ib);
  }
  return a.type.localeCompare(b.type);
}

export interface WorkingRrSet {
  rrset: RrSet;
  status: ChangeStatus | null;
  /** The server version, when it exists. */
  original: RrSet | null;
}

/** Merges server RRsets with pending changes for display. Deleted RRsets stay visible. */
export function workingRrSets(server: readonly RrSet[], changes: PendingChanges): WorkingRrSet[] {
  const rows: WorkingRrSet[] = [];
  const seen = new Set<string>();
  for (const rrset of server) {
    const key = rrKey(rrset.name, rrset.type);
    seen.add(key);
    const change = changes.get(key);
    if (!change) {
      rows.push({ rrset, status: null, original: rrset });
    } else {
      rows.push({ rrset: change.after ?? rrset, status: changeStatus(change), original: rrset });
    }
  }
  for (const change of changes.values()) {
    if (!seen.has(change.key) && change.after) {
      rows.push({ rrset: change.after, status: "added", original: null });
    }
  }
  return rows.sort((a, b) => compareRrSets(a.rrset, b.rrset));
}

/** Builds the single PATCH body for all pending changes. */
export function toPatchRequest(changes: PendingChanges): PatchRrSetsRequest {
  const rrsets = [...changes.values()]
    .sort((a, b) => compareRrSets(a, b))
    .map((change) => {
      if (change.after === null) {
        return {
          name: change.name,
          type: change.type,
          ttl: change.before?.ttl ?? 0,
          changetype: "DELETE" as const,
          records: [],
        };
      }
      return {
        name: change.after.name,
        type: change.after.type,
        ttl: change.after.ttl,
        changetype: "REPLACE" as const,
        records: change.after.records.map((r) => ({ content: r.content, disabled: r.disabled })),
      };
    });
  return { rrsets };
}
