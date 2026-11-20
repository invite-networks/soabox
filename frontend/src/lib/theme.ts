export type ThemeChoice = "system" | "light" | "dark";

const STORAGE_KEY = "soabox-theme";

function isThemeChoice(value: unknown): value is ThemeChoice {
  return value === "system" || value === "light" || value === "dark";
}

/** Reads the saved theme choice. Storage can be unavailable, so failures fall back to the system setting. */
export function readThemeChoice(): ThemeChoice {
  try {
    const value = window.localStorage.getItem(STORAGE_KEY);
    return isThemeChoice(value) ? value : "system";
  } catch {
    return "system";
  }
}

/** Applies a theme choice to the document. */
export function applyTheme(choice: ThemeChoice): void {
  const root = document.documentElement;
  if (choice === "system") {
    delete root.dataset["theme"];
  } else {
    root.dataset["theme"] = choice;
  }
}

/** Saves and applies a theme choice. Storage failures only skip persistence. */
export function saveThemeChoice(choice: ThemeChoice): void {
  applyTheme(choice);
  try {
    if (choice === "system") {
      window.localStorage.removeItem(STORAGE_KEY);
    } else {
      window.localStorage.setItem(STORAGE_KEY, choice);
    }
  } catch {
    // Storage is unavailable, for example in private browsing. The choice lasts for this page only.
  }
}
