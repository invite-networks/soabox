/**
 * DNS name and record content helpers used by the record editor.
 */

/** Starting types for template records. Zones use the list the backend reports. */
export const DEFAULT_FORWARD_TYPES = ["A", "AAAA", "CAA", "CNAME", "MX", "NS", "SRV", "TXT"] as const;

/** Record types offered in the settings screen. */
export const KNOWN_RECORD_TYPES = [
  "A",
  "AAAA",
  "AFSDB",
  "ALIAS",
  "CAA",
  "CERT",
  "CNAME",
  "DNAME",
  "DS",
  "HINFO",
  "HTTPS",
  "LOC",
  "MX",
  "NAPTR",
  "NS",
  "OPENPGPKEY",
  "PTR",
  "RP",
  "SPF",
  "SRV",
  "SSHFP",
  "SVCB",
  "TLSA",
  "TXT",
  "URI",
] as const;

/** Types that can be edited but never added or deleted from the editor. */
export const EDIT_ONLY_TYPES = new Set(["SOA"]);

/** Returns true for zones under in-addr.arpa. or ip6.arpa. */
export function isReverseZone(zoneName: string): boolean {
  const name = zoneName.toLowerCase();
  return name.endsWith("in-addr.arpa.") || name.endsWith("ip6.arpa.");
}

/** Ensures a name ends with a dot. */
export function ensureDot(name: string): string {
  return name.endsWith(".") ? name : `${name}.`;
}

/** Converts a name relative to the zone (`@`, `www`) to an absolute name. */
export function toAbsoluteName(relative: string, zoneName: string): string {
  const value = relative.trim();
  if (value === "" || value === "@") {
    return zoneName;
  }
  if (value.endsWith(".")) {
    return value;
  }
  return `${value}.${zoneName}`;
}

/** Converts an absolute name to one relative to the zone. The apex is `@`. */
export function toRelativeName(name: string, zoneName: string): string {
  if (name.toLowerCase() === zoneName.toLowerCase()) {
    return "@";
  }
  const suffix = `.${zoneName}`;
  if (name.toLowerCase().endsWith(suffix.toLowerCase())) {
    return name.slice(0, name.length - suffix.length);
  }
  return name;
}

/** Returns true when an absolute name is the zone apex or below it. */
export function isInZone(name: string, zoneName: string): boolean {
  const n = name.toLowerCase();
  const z = zoneName.toLowerCase();
  return n === z || n.endsWith(`.${z}`);
}

const LABEL = /^(?:\*|[A-Za-z0-9_](?:[A-Za-z0-9_-]{0,61}[A-Za-z0-9_])?)$/;

/** Validates a host name. A trailing dot is optional. A lone `.` is allowed when `allowRoot` is set. */
export function isHostname(value: string, allowRoot = false): boolean {
  if (value === ".") {
    return allowRoot;
  }
  const name = value.endsWith(".") ? value.slice(0, -1) : value;
  if (name.length === 0 || name.length > 253) {
    return false;
  }
  return name.split(".").every((label, index) => LABEL.test(label) && (label !== "*" || index === 0));
}

const IPV4 = /^(?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)$/;

export function isIPv4(value: string): boolean {
  return IPV4.test(value);
}

function isHexGroup(group: string): boolean {
  return /^[0-9A-Fa-f]{1,4}$/.test(group);
}

/** Validates an IPv6 address, including `::` compression and a trailing IPv4 part. */
export function isIPv6(value: string): boolean {
  if (!value.includes(":") || value.split("::").length > 2) {
    return false;
  }
  let text = value;
  let extra = 0;
  const lastColon = text.lastIndexOf(":");
  const tail = text.slice(lastColon + 1);
  if (tail.includes(".")) {
    if (!isIPv4(tail)) {
      return false;
    }
    text = `${text.slice(0, lastColon + 1)}0`;
    extra = 1;
  }
  const [head = "", rest] = text.split("::");
  const left = head === "" ? [] : head.split(":");
  const right = rest === undefined || rest === "" ? [] : rest.split(":");
  const groups = [...left, ...right];
  if (!groups.every(isHexGroup)) {
    return false;
  }
  const total = groups.length + extra;
  return rest === undefined ? total === 8 : total <= 7;
}

function isUint(value: string | undefined, max: number): boolean {
  if (value === undefined || !/^\d+$/.test(value)) {
    return false;
  }
  return Number(value) <= max;
}

const QUOTED_STRINGS = /^(?:"(?:[^"\\]|\\.)*"\s*)+$/;

function quote(text: string): string {
  const escaped = text.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  const chunks: string[] = [];
  let start = 0;
  while (start < escaped.length) {
    let end = Math.min(start + 255, escaped.length);
    // Do not split an escape sequence across two strings.
    if (escaped[end - 1] === "\\" && end < escaped.length) {
      end -= 1;
    }
    chunks.push(escaped.slice(start, end));
    start = end;
  }
  return chunks.length === 0 ? '""' : chunks.map((c) => `"${c}"`).join(" ");
}

/**
 * Normalizes record content into the presentation format PowerDNS expects.
 * Adds trailing dots to host names and quotes TXT and CAA values.
 */
export function normalizeContent(type: string, content: string): string {
  const value = content.trim();
  switch (type) {
    case "CNAME":
    case "NS":
    case "PTR":
    case "DNAME":
    case "ALIAS":
      return value === "" ? value : ensureDot(value);
    case "MX": {
      const m = /^(\d+)\s+(\S+)$/.exec(value);
      return m?.[1] && m[2] ? `${m[1]} ${m[2] === "." ? "." : ensureDot(m[2])}` : value;
    }
    case "SRV": {
      const m = /^(\d+)\s+(\d+)\s+(\d+)\s+(\S+)$/.exec(value);
      return m?.[4] ? `${m[1]} ${m[2]} ${m[3]} ${m[4] === "." ? "." : ensureDot(m[4])}` : value;
    }
    case "TXT":
    case "SPF":
      return value.startsWith('"') ? value : quote(value);
    case "CAA": {
      const m = /^(\d+)\s+(\S+)\s+(.+)$/.exec(value);
      if (m?.[3] && !m[3].startsWith('"')) {
        return `${m[1]} ${m[2]} ${quote(m[3])}`;
      }
      return value;
    }
    default:
      return value;
  }
}

/**
 * Validates record content for a type after normalization.
 *
 * @returns An error message, or `null` when the content is valid.
 */
export function validateContent(type: string, rawContent: string): string | null {
  const content = normalizeContent(type, rawContent);
  if (content === "") {
    return "Enter a value.";
  }
  switch (type) {
    case "A":
      return isIPv4(content) ? null : "Enter an IPv4 address, for example 192.0.2.10.";
    case "AAAA":
      return isIPv6(content) ? null : "Enter an IPv6 address, for example 2001:db8::10.";
    case "CNAME":
    case "NS":
    case "PTR":
    case "DNAME":
    case "ALIAS":
      return isHostname(content) ? null : "Enter a host name, for example host.example.com.";
    case "MX": {
      const m = /^(\d+)\s+(\S+)$/.exec(content);
      if (!m || !isUint(m[1], 65535) || !isHostname(m[2] ?? "", true)) {
        return "Enter a priority and a host name, for example 10 mail.example.com.";
      }
      return null;
    }
    case "SRV": {
      const m = /^(\d+)\s+(\d+)\s+(\d+)\s+(\S+)$/.exec(content);
      if (!m || !isUint(m[1], 65535) || !isUint(m[2], 65535) || !isUint(m[3], 65535) || !isHostname(m[4] ?? "", true)) {
        return "Enter priority, weight, port, and target, for example 10 5 5060 sip.example.com.";
      }
      return null;
    }
    case "TXT":
    case "SPF":
      return QUOTED_STRINGS.test(content) ? null : "Enter text. Quoted values must close their quotes.";
    case "CAA": {
      const m = /^(\d+)\s+([A-Za-z0-9]+)\s+(".*")$/.exec(content);
      if (!m || !isUint(m[1], 255) || !QUOTED_STRINGS.test(m[3] ?? "")) {
        return 'Enter flags, tag, and value, for example 0 issue "ca.example.net".';
      }
      return null;
    }
    case "SOA": {
      const m = /^(\S+)\s+(\S+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)$/.exec(content);
      if (!m || !isHostname(m[1] ?? "") || !isHostname(m[2] ?? "")) {
        return "Enter primary server, contact, serial, refresh, retry, expire, and minimum.";
      }
      return null;
    }
    default:
      return null;
  }
}

/** Validates a TTL value in seconds. */
export function validateTtl(ttl: number): string | null {
  if (!Number.isInteger(ttl) || ttl < 0 || ttl > 2147483647) {
    return "Enter a whole number of seconds.";
  }
  return null;
}

/** Validates a zone name. */
export function validateZoneName(name: string): string | null {
  return isHostname(name.trim()) ? null : "Enter a valid zone name, for example example.com.";
}
