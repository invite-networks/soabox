import { afterEach, describe, expect, it, vi } from "vitest";
import { applyTheme, readThemeChoice, saveThemeChoice } from "./theme";

describe("theme", () => {
  afterEach(() => vi.restoreAllMocks());

  it("saves, applies, and reads the choice", () => {
    expect(readThemeChoice()).toBe("system");
    saveThemeChoice("dark");
    expect(document.documentElement.dataset["theme"]).toBe("dark");
    expect(readThemeChoice()).toBe("dark");
    saveThemeChoice("system");
    expect(document.documentElement.dataset["theme"]).toBeUndefined();
    expect(readThemeChoice()).toBe("system");
    applyTheme("light");
    expect(document.documentElement.dataset["theme"]).toBe("light");
  });

  it("ignores invalid stored values", () => {
    window.localStorage.setItem("soabox-theme", "neon");
    expect(readThemeChoice()).toBe("system");
  });

  it("survives unavailable storage", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("denied");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("denied");
    });
    expect(readThemeChoice()).toBe("system");
    expect(() => saveThemeChoice("dark")).not.toThrow();
    expect(document.documentElement.dataset["theme"]).toBe("dark");
  });
});
