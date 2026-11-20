import { describe, expect, it } from "vitest";
import { dateToUnix, formatDuration, formatTime, formatTtl, joinList, splitList, unixToDate } from "./format";

describe("format", () => {
  it("formats durations", () => {
    expect(formatDuration(59)).toBe("0m");
    expect(formatDuration(3700)).toBe("1h 1m");
    expect(formatDuration(90_000)).toBe("1d 1h");
  });

  it("formats times", () => {
    expect(formatTime(null)).toBe("Never");
    expect(formatTime(0)).toMatch(/19(69|70)/);
  });

  it("round trips dates", () => {
    const start = dateToUnix("2026-11-19");
    const end = dateToUnix("2026-11-19", true);
    expect(start).toBeDefined();
    expect((end ?? 0) - (start ?? 0)).toBe(86_399);
    expect(unixToDate(start)).toBe("2026-11-19");
    expect(unixToDate(null)).toBe("");
    expect(dateToUnix("bad")).toBeUndefined();
    expect(dateToUnix(undefined)).toBeUndefined();
  });

  it("splits and joins lists", () => {
    expect(splitList("a\n b ,c\n\n")).toEqual(["a", "b", "c"]);
    expect(joinList(["a", "b"])).toBe("a\nb");
  });
});

describe("formatTtl", () => {
  it("uses the largest whole unit", () => {
    expect(formatTtl(300)).toBe("5m");
    expect(formatTtl(3600)).toBe("1h");
    expect(formatTtl(86400)).toBe("1d");
    expect(formatTtl(604800)).toBe("1w");
  });

  it("keeps odd values in seconds", () => {
    expect(formatTtl(0)).toBe("0s");
    expect(formatTtl(90)).toBe("90s");
    expect(formatTtl(3601)).toBe("3601s");
  });
});
