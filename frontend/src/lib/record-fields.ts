/**
 * Per-type field definitions for the record editor.
 *
 * Each supported record type is described as a list of named fields. The editor
 * renders one input per field, restricts what can be typed, validates each field,
 * and composes the PowerDNS presentation format from the parts.
 *
 * Host names follow zone file rules: a name ending in a dot is absolute, `@` is
 * the zone itself, and any other name is relative to the zone.
 */
import { isHostname, isIPv4, isIPv6 } from "./dns";

export type FieldKind =
  | "uint"
  | "ipv4"
  | "ipv6"
  | "host"
  | "mailbox"
  | "text"
  | "hex"
  | "base64"
  | "token"
  | "select"
  | "params"
  | "raw";

export interface FieldOption {
  id: string;
  label: string;
}

export interface FieldDef {
  key: string;
  label: string;
  kind: FieldKind;
  /** Largest value for `uint` fields. */
  max?: number;
  options?: readonly FieldOption[];
  /** Host fields that accept `.` for "no target". */
  allowRoot?: boolean;
  placeholder?: string;
  /** The field takes every remaining token. Only valid on the last field. */
  rest?: boolean;
  optional?: boolean;
  multiline?: boolean;
  /** Layout hint: wide fields grow to fill the row. */
  wide?: boolean;
  /** Unit shown after the input, such as "sec". */
  unit?: string;
}

const U8 = 255;
const U16 = 65535;
const U32 = 4294967295;

const uint = (key: string, label: string, max: number, placeholder?: string): FieldDef => ({
  key,
  label,
  kind: "uint",
  max,
  ...(placeholder ? { placeholder } : {}),
});

const host = (key: string, label: string, extra: Partial<FieldDef> = {}): FieldDef => ({
  key,
  label,
  kind: "host",
  wide: true,
  ...extra,
});

const DNSSEC_ALGORITHMS: readonly FieldOption[] = [
  { id: "5", label: "5 RSASHA1" },
  { id: "7", label: "7 RSASHA1-NSEC3" },
  { id: "8", label: "8 RSASHA256" },
  { id: "10", label: "10 RSASHA512" },
  { id: "13", label: "13 ECDSAP256SHA256" },
  { id: "14", label: "14 ECDSAP384SHA384" },
  { id: "15", label: "15 ED25519" },
  { id: "16", label: "16 ED448" },
];

const SVCB_FIELDS: readonly FieldDef[] = [
  uint("priority", "Priority", U16, "1"),
  host("target", "Target", { allowRoot: true, placeholder: "." }),
  {
    key: "params",
    label: "Parameters",
    kind: "params",
    rest: true,
    optional: true,
    wide: true,
    placeholder: "alpn=h2,h3",
  },
];

/** Field layouts for every supported record type. */
export const RECORD_FIELDS: Readonly<Record<string, readonly FieldDef[]>> = {
  A: [{ key: "address", label: "IPv4 address", kind: "ipv4", wide: true, placeholder: "192.0.2.10" }],
  AAAA: [{ key: "address", label: "IPv6 address", kind: "ipv6", wide: true, placeholder: "2001:db8::10" }],
  AFSDB: [uint("subtype", "Subtype", U16, "1"), host("host", "Host name", { placeholder: "afsdb" })],
  ALIAS: [host("target", "Target", { placeholder: "target.example.net." })],
  CAA: [
    uint("flags", "Flags", U8, "0"),
    {
      key: "tag",
      label: "Tag",
      kind: "select",
      options: [
        { id: "issue", label: "issue" },
        { id: "issuewild", label: "issuewild" },
        { id: "iodef", label: "iodef" },
        { id: "issuemail", label: "issuemail" },
      ],
    },
    { key: "value", label: "Value", kind: "text", wide: true, placeholder: "letsencrypt.org" },
  ],
  CERT: [
    { key: "type", label: "Certificate type", kind: "token", placeholder: "PKIX" },
    uint("keytag", "Key tag", U16, "0"),
    { key: "algorithm", label: "Algorithm", kind: "token", placeholder: "8" },
    { key: "certificate", label: "Certificate", kind: "base64", rest: true, wide: true, multiline: true },
  ],
  CNAME: [host("target", "Target", { placeholder: "target.example.net." })],
  DNAME: [host("target", "Target", { placeholder: "example.net." })],
  DS: [
    uint("keytag", "Key tag", U16, "12345"),
    { key: "algorithm", label: "Algorithm", kind: "select", options: DNSSEC_ALGORITHMS },
    {
      key: "digesttype",
      label: "Digest type",
      kind: "select",
      options: [
        { id: "1", label: "1 SHA-1" },
        { id: "2", label: "2 SHA-256" },
        { id: "4", label: "4 SHA-384" },
      ],
    },
    { key: "digest", label: "Digest", kind: "hex", rest: true, wide: true },
  ],
  HINFO: [
    { key: "cpu", label: "CPU", kind: "text", placeholder: "x86_64" },
    { key: "os", label: "Operating system", kind: "text", wide: true, placeholder: "Linux" },
  ],
  HTTPS: SVCB_FIELDS,
  LOC: [
    {
      key: "location",
      label: "Location",
      kind: "raw",
      rest: true,
      wide: true,
      placeholder: "51 30 12.748 N 0 7 39.611 W 0.00m",
    },
  ],
  MX: [
    uint("priority", "Priority", U16, "10"),
    host("target", "Mail server", { allowRoot: true, placeholder: "mail" }),
  ],
  NAPTR: [
    uint("order", "Order", U16, "100"),
    uint("preference", "Preference", U16, "10"),
    { key: "flags", label: "Flags", kind: "text", placeholder: "U" },
    { key: "service", label: "Service", kind: "text", placeholder: "E2U+sip" },
    { key: "regexp", label: "Regular expression", kind: "text", wide: true, optional: true },
    host("replacement", "Replacement", { allowRoot: true, placeholder: "." }),
  ],
  NS: [host("target", "Name server", { placeholder: "ns1.example.net." })],
  OPENPGPKEY: [{ key: "key", label: "Public key", kind: "base64", rest: true, wide: true, multiline: true }],
  PTR: [host("target", "Host name", { placeholder: "host.example.com." })],
  RP: [
    {
      key: "mailbox",
      label: "Mailbox",
      kind: "mailbox",
      allowRoot: true,
      wide: true,
      placeholder: "admin@example.com",
    },
    host("txt", "TXT record name", { allowRoot: true, placeholder: "." }),
  ],
  SOA: [
    host("primary", "Primary name server", { placeholder: "ns1.example.net." }),
    { key: "contact", label: "Contact email", kind: "mailbox", wide: true, placeholder: "hostmaster@example.com" },
    uint("serial", "Serial", U32),
    { ...uint("refresh", "Refresh", U32, "10800"), unit: "sec" },
    { ...uint("retry", "Retry", U32, "3600"), unit: "sec" },
    { ...uint("expire", "Expire", U32, "604800"), unit: "sec" },
    { ...uint("minimum", "Negative TTL", U32, "3600"), unit: "sec" },
  ],
  SPF: [{ key: "text", label: "SPF policy", kind: "text", rest: true, wide: true, placeholder: "v=spf1 mx -all" }],
  SRV: [
    uint("priority", "Priority", U16, "10"),
    uint("weight", "Weight", U16, "5"),
    uint("port", "Port", U16, "443"),
    host("target", "Target", { allowRoot: true, placeholder: "sip" }),
  ],
  SSHFP: [
    {
      key: "algorithm",
      label: "Algorithm",
      kind: "select",
      options: [
        { id: "1", label: "1 RSA" },
        { id: "2", label: "2 DSA" },
        { id: "3", label: "3 ECDSA" },
        { id: "4", label: "4 Ed25519" },
        { id: "6", label: "6 Ed448" },
      ],
    },
    {
      key: "fptype",
      label: "Fingerprint type",
      kind: "select",
      options: [
        { id: "1", label: "1 SHA-1" },
        { id: "2", label: "2 SHA-256" },
      ],
    },
    { key: "fingerprint", label: "Fingerprint", kind: "hex", rest: true, wide: true },
  ],
  SVCB: SVCB_FIELDS,
  TLSA: [
    {
      key: "usage",
      label: "Usage",
      kind: "select",
      options: [
        { id: "0", label: "0 PKIX-TA" },
        { id: "1", label: "1 PKIX-EE" },
        { id: "2", label: "2 DANE-TA" },
        { id: "3", label: "3 DANE-EE" },
      ],
    },
    {
      key: "selector",
      label: "Selector",
      kind: "select",
      options: [
        { id: "0", label: "0 Full certificate" },
        { id: "1", label: "1 Public key" },
      ],
    },
    {
      key: "matching",
      label: "Matching type",
      kind: "select",
      options: [
        { id: "0", label: "0 Exact" },
        { id: "1", label: "1 SHA-256" },
        { id: "2", label: "2 SHA-512" },
      ],
    },
    { key: "data", label: "Certificate data", kind: "hex", rest: true, wide: true },
  ],
  TXT: [
    {
      key: "text",
      label: "Text",
      kind: "text",
      rest: true,
      wide: true,
      multiline: true,
      placeholder: "v=spf1 mx -all",
    },
  ],
  URI: [
    uint("priority", "Priority", U16, "10"),
    uint("weight", "Weight", U16, "1"),
    { key: "target", label: "Target URI", kind: "text", wide: true, placeholder: "https://www.example.com/" },
  ],
};

/** A single free-form field, used for unknown types and values the typed fields cannot represent. */
export const RAW_FIELDS: readonly FieldDef[] = [{ key: "value", label: "Value", kind: "raw", rest: true, wide: true }];

export function fieldsFor(type: string): readonly FieldDef[] {
  return RECORD_FIELDS[type] ?? RAW_FIELDS;
}

export function emptyFields(type: string): string[] {
  return fieldsFor(type).map((f) => (f.kind === "select" ? (f.options?.[0]?.id ?? "") : ""));
}

// Tokens -------------------------------------------------------------------

interface Token {
  text: string;
  quoted: boolean;
}

/** Splits presentation format into words and quoted strings. Returns null on unbalanced quotes. */
export function tokenize(content: string): Token[] | null {
  const tokens: Token[] = [];
  let i = 0;
  while (i < content.length) {
    const ch = content[i];
    if (ch === " " || ch === "\t" || ch === "\n") {
      i += 1;
      continue;
    }
    if (ch === '"') {
      let j = i + 1;
      while (j < content.length && content[j] !== '"') {
        j += content[j] === "\\" ? 2 : 1;
      }
      if (j >= content.length) {
        return null;
      }
      tokens.push({ text: content.slice(i, j + 1), quoted: true });
      i = j + 1;
      continue;
    }
    let j = i;
    while (j < content.length && !/\s/.test(content[j] ?? "")) {
      j += 1;
    }
    tokens.push({ text: content.slice(i, j), quoted: false });
    i = j;
  }
  return tokens;
}

function unquote(token: Token): string {
  if (!token.quoted) {
    return token.text;
  }
  return token.text.slice(1, -1).replace(/\\(.)/g, "$1");
}

/** Quotes text, splitting it into 255 character strings as DNS requires. */
export function quoteText(text: string): string {
  const escaped = text.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  const chunks: string[] = [];
  let start = 0;
  while (start < escaped.length) {
    let end = Math.min(start + 255, escaped.length);
    if (escaped[end - 1] === "\\" && end < escaped.length) {
      end -= 1;
    }
    chunks.push(escaped.slice(start, end));
    start = end;
  }
  return chunks.length === 0 ? '""' : chunks.map((c) => `"${c}"`).join(" ");
}

// Host names ---------------------------------------------------------------

function isHostKind(field: FieldDef): boolean {
  return field.kind === "host" || field.kind === "mailbox";
}

/**
 * Shows an absolute name relative to the zone where possible.
 * With no zone (templates), the stored text is shown as is.
 */
export function relativeHost(value: string, zone: string | null): string {
  if (zone === null || value === ".") {
    return value;
  }
  const v = value.toLowerCase();
  const z = zone.toLowerCase();
  if (v === z) {
    return "@";
  }
  if (v.endsWith(`.${z}`)) {
    return value.slice(0, value.length - z.length - 1);
  }
  return value;
}

/**
 * Turns what the user typed into the stored name. In a zone the result is absolute.
 * In a template (no zone) relative names are kept and expanded when the zone is created.
 */
export function absoluteHost(value: string, zone: string | null, mailbox = false): string {
  let v = value.trim();
  if (mailbox && v.includes("@") && v !== "@") {
    // An email address: the first @ becomes a dot and the name is absolute.
    const at = v.indexOf("@");
    v = `${v.slice(0, at).replace(/\./g, "\\.")}.${v.slice(at + 1)}`;
    return v.endsWith(".") ? v : `${v}.`;
  }
  if (v.endsWith(".")) {
    return v;
  }
  if (zone === null) {
    return v;
  }
  return v === "@" ? zone : `${v}.${zone}`;
}

/**
 * Shows a DNS mailbox (`hostmaster.example.com.`) as an email address (`hostmaster@example.com`).
 * The first unescaped dot separates the local part. `.` and names without a domain are kept.
 */
export function mailboxToEmail(value: string): string {
  if (value === "." || value.includes("@")) {
    return value;
  }
  let i = 0;
  while (i < value.length) {
    if (value[i] === "\\") {
      i += 2;
      continue;
    }
    if (value[i] === ".") {
      break;
    }
    i += 1;
  }
  const local = value.slice(0, i).replace(/\\\./g, ".");
  const domain = value.slice(i + 1).replace(/\.$/, "");
  return local && domain ? `${local}@${domain}` : value;
}

/** True when the typed host name will have the zone appended. */
export function getsZoneSuffix(field: FieldDef, value: string): boolean {
  if (!isHostKind(field)) {
    return false;
  }
  const v = value.trim();
  if (v === "" || v === "@" || v.endsWith(".")) {
    return false;
  }
  return !(field.kind === "mailbox" && v.includes("@"));
}

// Parse and compose --------------------------------------------------------

/**
 * Splits stored content into field values for the editor.
 * Returns null when the content does not fit the typed fields.
 */
export function parseFields(type: string, content: string, zone: string | null): string[] | null {
  const defs = RECORD_FIELDS[type];
  if (!defs) {
    return null;
  }
  const tokens = tokenize(content.trim());
  if (!tokens) {
    return null;
  }
  const values: string[] = [];
  let index = 0;
  for (const def of defs) {
    if (def.rest) {
      const restTokens = tokens.slice(index);
      index = tokens.length;
      if (def.kind === "text") {
        if (restTokens.length === 0 || !restTokens.every((t) => t.quoted)) {
          return null;
        }
        values.push(restTokens.map(unquote).join(""));
      } else {
        if (restTokens.length === 0 && !def.optional) {
          return null;
        }
        if (restTokens.some((t) => t.quoted) && def.kind !== "params" && def.kind !== "raw") {
          return null;
        }
        values.push(restTokens.map((t) => t.text).join(" "));
      }
      continue;
    }
    const token = tokens[index];
    index += 1;
    if (!token) {
      return null;
    }
    if (def.kind === "text") {
      values.push(unquote(token));
    } else if (token.quoted) {
      return null;
    } else if (def.kind === "mailbox") {
      values.push(mailboxToEmail(token.text));
    } else if (isHostKind(def)) {
      values.push(relativeHost(token.text, zone));
    } else {
      values.push(token.text);
    }
  }
  if (index < tokens.length) {
    return null;
  }
  return values;
}

/** Builds the stored presentation format from field values. */
export function composeFields(type: string, values: readonly string[], zone: string | null): string {
  const defs = fieldsFor(type);
  const parts: string[] = [];
  defs.forEach((def, i) => {
    const value = (values[i] ?? "").trim();
    if (value === "" && def.optional && def.kind !== "text") {
      return;
    }
    switch (def.kind) {
      case "text":
        parts.push(quoteText(values[i] ?? ""));
        break;
      case "host":
        parts.push(absoluteHost(value, zone));
        break;
      case "mailbox":
        parts.push(absoluteHost(value, zone, true));
        break;
      case "hex":
      case "base64":
        parts.push(value.replace(/\s+/g, ""));
        break;
      default:
        parts.push(value);
    }
  });
  return parts.join(" ");
}

// Input restriction and validation ----------------------------------------

const ALLOWED: Partial<Record<FieldKind, RegExp>> = {
  uint: /[^0-9]/g,
  ipv4: /[^0-9.]/g,
  ipv6: /[^0-9A-Fa-f:.]/g,
  host: /[^A-Za-z0-9._*@-]/g,
  mailbox: /[^A-Za-z0-9._*@+-]/g,
  hex: /[^0-9A-Fa-f]/g,
  base64: /[^A-Za-z0-9+/=\s]/g,
  token: /[^A-Za-z0-9-]/g,
};

/** Removes characters a field can never contain. */
export function restrictInput(field: FieldDef, value: string): string {
  const pattern = ALLOWED[field.kind];
  return pattern ? value.replace(pattern, "") : value;
}

/** Input mode hint for on-screen keyboards. */
export function inputModeFor(field: FieldDef): "numeric" | "text" {
  return field.kind === "uint" ? "numeric" : "text";
}

const EMAIL_LOCAL = /^[A-Za-z0-9._+-]+$/;

/** Checks one field. Returns an error message, or null when it is valid. */
export function validateField(field: FieldDef, raw: string, zone: string | null): string | null {
  const value = field.kind === "text" ? raw : raw.trim();
  if (value.trim() === "") {
    return field.optional ? null : `${field.label} is required.`;
  }
  switch (field.kind) {
    case "uint":
      return /^\d+$/.test(value) && Number(value) <= (field.max ?? U32)
        ? null
        : `Enter a number from 0 to ${field.max ?? U32}.`;
    case "ipv4":
      return isIPv4(value) ? null : "Enter an IPv4 address, for example 192.0.2.10.";
    case "ipv6":
      return isIPv6(value) ? null : "Enter an IPv6 address, for example 2001:db8::10.";
    case "mailbox":
      if (value.includes("@") && value !== "@") {
        const at = value.indexOf("@");
        return EMAIL_LOCAL.test(value.slice(0, at)) && isHostname(value.slice(at + 1))
          ? null
          : "Enter an email address or a host name.";
      }
      return validateHost(field, value, zone);
    case "host":
      return validateHost(field, value, zone);
    case "hex":
      return /^[0-9A-Fa-f]+$/.test(value) ? null : "Use hexadecimal characters 0 to 9 and A to F.";
    case "base64":
      return /^[A-Za-z0-9+/\s]+=*$/.test(value) ? null : "Enter base64 encoded data.";
    case "token":
      return /^[A-Za-z0-9-]+$/.test(value) ? null : "Use letters, digits, and hyphens.";
    case "select":
      return /^[A-Za-z0-9]+$/.test(value) ? null : "Choose a value.";
    default:
      return null;
  }
}

function validateHost(field: FieldDef, value: string, zone: string | null): string | null {
  if (value === ".") {
    return field.allowRoot ? null : "Enter a host name.";
  }
  if (value === "@") {
    return null;
  }
  const absolute = absoluteHost(value, zone);
  return isHostname(absolute) ? null : "Enter a host name, for example host or host.example.net.";
}

// Display ------------------------------------------------------------------

export interface ValuePart {
  text: string;
  /** True for the zone suffix that relative names resolve to. */
  suffix?: boolean;
}

/**
 * Splits stored content for display, marking the zone suffix on host names inside the zone
 * so readers can see what relative names resolve to.
 */
export function displayParts(type: string, content: string, zone: string): ValuePart[] {
  const defs = RECORD_FIELDS[type];
  const tokens = defs ? tokenize(content) : null;
  if (!defs || !tokens) {
    return [{ text: content }];
  }
  const z = zone.toLowerCase();
  const out: ValuePart[] = [];
  const push = (part: ValuePart) => {
    const last = out.at(-1);
    if (last && !last.suffix && !part.suffix) {
      last.text += part.text;
    } else if (part.text !== "") {
      out.push(part);
    }
  };
  tokens.forEach((token, i) => {
    const def = defs[i];
    const sep = i === 0 ? "" : " ";
    const lower = token.text.toLowerCase();
    const inZone = lower === z || lower.endsWith(`.${z}`);
    // Mailboxes (SOA and RP contacts) are email addresses, so they are shown as stored.
    if (def && !def.rest && def.kind === "host" && !token.quoted && inZone) {
      const head = lower === z ? "" : token.text.slice(0, token.text.length - z.length - 1);
      push({ text: `${sep}${head}` });
      push({ text: head === "" ? zone : `.${zone}`, suffix: true });
      return;
    }
    push({ text: `${sep}${token.text}` });
  });
  return out;
}
