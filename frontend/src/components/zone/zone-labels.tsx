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
