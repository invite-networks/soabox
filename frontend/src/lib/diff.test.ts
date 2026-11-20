import { describe, expect, it } from "vitest";
import { compareLines, detailText, parseDiff, snapshotLines, stringify } from "./diff";

describe("history diff", () => {
  it("renders record changes with TTL and disabled values", () => {
    const entries = parseDiff([
      {
        name: "www.example.com.",
        type: "A",
        before: {
          ttl: 300,
          records: [
            { content: "a", disabled: false },
            { content: "b", disabled: false },
          ],
        },
        after: {
          ttl: 300,
          records: [
            { content: "b", disabled: false },
            { content: "c", disabled: true },
          ],
        },
      },
      {
        name: "new.example.com.",
        type: "TXT",
        before: null,
        after: { ttl: 60, records: [{ content: '"x"', disabled: false }] },
      },
      {
        name: "gone.example.com.",
        type: "A",
        before: { ttl: 60, records: [{ content: "d", disabled: false }] },
        after: null,
      },
    ]);
    expect(entries.map((e) => e.title)).toEqual(["www.example.com. A", "new.example.com. TXT", "gone.example.com. A"]);
    expect(entries[0]?.lines).toEqual([
      { kind: "unchanged", text: "TTL 300" },
      { kind: "removed", text: "a" },
      { kind: "unchanged", text: "b" },
      { kind: "added", text: "c (disabled)" },
    ]);
    expect(entries[1]?.lines.every((l) => l.kind === "added")).toBe(true);
    expect(entries[2]?.lines.every((l) => l.kind === "removed")).toBe(true);
  });

  it("renders zone setting changes", () => {
    const entries = parseDiff({
      zone: [
        { field: "kind", before: "Native", after: "Master" },
        { field: "masters", before: [], after: ["192.0.2.53"] },
        { field: "api_rectify", before: true, after: true },
        { field: "account", before: null, after: "Ops" },
      ],
    });
    expect(entries.map((e) => e.title)).toEqual(["kind", "masters", "api_rectify", "account"]);
    expect(entries[0]?.lines).toEqual([
      { kind: "removed", text: "Native" },
      { kind: "added", text: "Master" },
    ]);
    expect(entries[1]?.lines.at(-1)).toEqual({ kind: "added", text: '["192.0.2.53"]' });
    expect(entries[2]?.lines).toEqual([{ kind: "unchanged", text: "true" }]);
    expect(entries[3]?.lines[0]).toEqual({ kind: "removed", text: "none" });
  });

  it("falls back to JSON for unknown shapes", () => {
    expect(parseDiff(null)).toEqual([]);
    expect(parseDiff([{ unexpected: true }])[0]?.title).toBe("Change");
    expect(
      parseDiff({ other: 1 })[0]
        ?.lines.map((l) => l.text)
        .join(""),
    ).toContain('"other": 1');
    expect(parseDiff("raw")[0]?.lines[0]?.text).toBe('"raw"');
  });

  it("extracts failure errors and requests from detail", () => {
    expect(detailText(null)).toEqual({ error: null, request: null });
    expect(detailText("plain")).toEqual({ error: null, request: "plain" });
    expect(detailText({ request: { a: 1 }, error: "Boom" })).toEqual({ error: "Boom", request: '{\n  "a": 1\n}' });
    expect(detailText({ error: { code: 1 } })).toEqual({ error: '{"code":1}', request: null });
    expect(detailText({ name: "x" }).request).toContain('"name": "x"');
  });

  it("has small helpers", () => {
    expect(snapshotLines(null)).toEqual([]);
    expect(compareLines(["a"], ["a"])).toEqual([{ kind: "unchanged", text: "a" }]);
    expect(stringify(undefined)).toBe("none");
    expect(stringify(3)).toBe("3");
  });
});
