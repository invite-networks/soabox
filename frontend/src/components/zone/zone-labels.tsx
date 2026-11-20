import { ShieldCheck, ShieldOff } from "lucide-react";
import type { ZoneKind } from "../../api/types";
import { Badge } from "../ui";

/** Display names for PowerDNS zone kinds. */
export const KIND_LABELS: Record<ZoneKind, string> = {
  Native: "Native",
  Master: "Primary",
  Slave: "Secondary",
};

/** DNSSEC state as icon plus text. */
export function DnssecBadge({ enabled }: { enabled: boolean }) {
  return enabled ? (
    <Badge tone="success">
      <ShieldCheck aria-hidden />
      Signed
    </Badge>
  ) : (
    <Badge>
      <ShieldOff aria-hidden />
      Unsigned
    </Badge>
  );
}

const TYPE_FAMILIES: [readonly string[], string][] = [
  [["A", "AAAA"], "bg-accent-soft text-accent-text"],
  [["CNAME", "ALIAS", "DNAME", "PTR"], "bg-violet-soft text-violet"],
  [["MX", "SPF"], "bg-pink-soft text-pink"],
  [["TXT", "SRV", "NAPTR", "HTTPS", "SVCB", "URI", "LOC", "HINFO", "RP"], "bg-cyan-soft text-cyan"],
  [["SOA", "NS"], "bg-warning-soft text-warning"],
  [["DS", "DNSKEY", "CDS", "CDNSKEY", "CAA", "TLSA", "SSHFP", "CERT", "OPENPGPKEY"], "bg-success-soft text-success"],
];

/** A record type as a colored pill. Color groups related types; the type name is always shown. */
export function RecordType({ type }: { type: string }) {
  const tone = TYPE_FAMILIES.find(([types]) => types.includes(type))?.[1] ?? "bg-surface-muted text-fg-muted";
  return (
    <span
      className={`inline-flex min-w-12 justify-center rounded-md px-1.5 py-0.5 font-mono text-[11px] font-semibold tracking-wide ${tone}`}
    >
      {type}
    </span>
  );
}
