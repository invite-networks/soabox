const dateTime = new Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
});

/** Formats Unix seconds as a local date and time. */
export function formatTime(unix: number | null | undefined): string {
  if (unix === null || unix === undefined) {
    return "Never";
  }
  return dateTime.format(new Date(unix * 1000));
}

/** Formats a number of seconds as a short duration such as `3d 4h`. */
export function formatDuration(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const days = Math.floor(s / 86400);
  const hours = Math.floor((s % 86400) / 3600);
  const minutes = Math.floor((s % 3600) / 60);
  if (days > 0) {
    return `${days}d ${hours}h`;
  }
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  return `${minutes}m`;
}

/** Converts a `YYYY-MM-DD` date input to Unix seconds at local start or end of day. */
export function dateToUnix(value: string | undefined, endOfDay = false): number | undefined {
  if (!value || !/^\d{4}-\d{2}-\d{2}$/.test(value)) {
    return undefined;
  }
  const [y, m, d] = value.split("-").map(Number) as [number, number, number];
  const date = endOfDay ? new Date(y, m - 1, d, 23, 59, 59) : new Date(y, m - 1, d, 0, 0, 0);
  return Math.floor(date.getTime() / 1000);
}

/** Converts Unix seconds to a `YYYY-MM-DD` date input value in local time. */
export function unixToDate(unix: number | null | undefined): string {
  if (unix === null || unix === undefined) {
    return "";
  }
  const d = new Date(unix * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** Splits a multi-line or comma separated text into trimmed, non-empty items. */
export function splitList(text: string): string[] {
  return text
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

/** Joins items one per line for a text area. */
export function joinList(items: readonly string[]): string {
  return items.join("\n");
}

/** Formats a TTL in seconds the way operators say it, such as `5m`, `1h`, or `1d`. Odd values stay in seconds. */
export function formatTtl(seconds: number): string {
  const units: [number, string][] = [
    [604800, "w"],
    [86400, "d"],
    [3600, "h"],
    [60, "m"],
  ];
  for (const [size, suffix] of units) {
    if (seconds >= size && seconds % size === 0) {
      return `${seconds / size}${suffix}`;
    }
  }
  return `${seconds}s`;
}
