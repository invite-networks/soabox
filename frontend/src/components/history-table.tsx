import { CheckCircle2, ChevronDown, ChevronRight, XCircle } from "lucide-react";
import { Fragment, useState } from "react";
import type { HistoryEntry } from "../api/types";
import { formatTime } from "../lib/format";
import { DiffView } from "./diff-view";
import { Badge, Button, Link, Table, TBody, Td, THead, Th, Tr } from "./ui";

function Outcome({ entry }: { entry: HistoryEntry }) {
  return entry.outcome === "success" ? (
    <Badge tone="success">
      <CheckCircle2 aria-hidden />
      Success
    </Badge>
  ) : (
    <Badge tone="danger">
      <XCircle aria-hidden />
      Failed
      {entry.upstream_status ? ` (${entry.upstream_status})` : ""}
    </Badge>
  );
}

const OPERATION_LABELS: Record<string, string> = {
  "zone.create": "Created zone",
  "zone.update": "Updated zone",
  "zone.delete": "Deleted zone",
  "zone.notify": "Sent NOTIFY",
  "zone.rectify": "Rectified zone",
  "zone.axfr_retrieve": "Retrieved zone",
  "rrsets.patch": "Changed records",
  "cryptokey.create": "Added DNSSEC key",
  "cryptokey.update": "Updated DNSSEC key",
  "cryptokey.delete": "Removed DNSSEC key",
  "metadata.update": "Updated metadata",
  "metadata.delete": "Removed metadata",
  "api_key.create": "Created API key",
  "api_key.update": "Updated API key",
  "api_key.enable": "Enabled API key",
  "api_key.disable": "Disabled API key",
  "api_key.delete": "Deleted API key",
  "user.create": "Created user",
  "user.update": "Updated user",
  "user.delete": "Deleted user",
  "user.password_reset": "Reset password",
  "user.totp_reset": "Reset two-factor",
  "account.create": "Created account",
  "account.update": "Updated account",
  "account.delete": "Deleted account",
  "template.create": "Created template",
  "template.update": "Updated template",
  "template.delete": "Deleted template",
  "settings.update": "Changed settings",
  "settings.oidc_update": "Changed single sign-on",
};

/** A readable operation name with the raw operation code beneath it. */
function Operation({ code }: { code: string }) {
  const label = OPERATION_LABELS[code];
  if (!label) {
    return <span className="font-mono text-[13px]">{code}</span>;
  }
  return (
    <span className="flex flex-col leading-tight">
      <span className="text-fg">{label}</span>
      <span className="font-mono text-[11px] text-fg-muted">{code}</span>
    </span>
  );
}

const ACTOR_TYPES: Record<string, string> = { user: "user", api_key: "API key", system: "system" };

export interface HistoryTableProps {
  entries: readonly HistoryEntry[];
  label: string;
  /** Show the zone column. Off inside a zone. */
  showZone?: boolean;
}

/** History entries with an expandable change view per row. */
export function HistoryTable({ entries, label, showZone = true }: HistoryTableProps) {
  const [open, setOpen] = useState<Set<number>>(new Set());
  const toggle = (id: number) =>
    setOpen((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  const columns = showZone ? 6 : 5;
  return (
    <Table label={label}>
      <THead>
        <tr>
          <Th hidden>Details</Th>
          <Th>Time</Th>
          <Th>Actor</Th>
          <Th>Operation</Th>
          {showZone && <Th>Zone</Th>}
          <Th>Outcome</Th>
        </tr>
      </THead>
      <TBody>
        {entries.map((entry) => {
          const isOpen = open.has(entry.id);
          return (
            <Fragment key={entry.id}>
              <Tr>
                <Td>
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-expanded={isOpen}
                    aria-label={isOpen ? "Hide changes" : "Show changes"}
                    onPress={() => toggle(entry.id)}
                  >
                    {isOpen ? <ChevronDown aria-hidden /> : <ChevronRight aria-hidden />}
                  </Button>
                </Td>
                <Td>{formatTime(entry.at)}</Td>
                <Td>
                  <span className="text-fg">{entry.actor_name}</span>{" "}
                  <span className="text-xs text-fg-muted">{ACTOR_TYPES[entry.actor_type] ?? entry.actor_type}</span>
                </Td>
                <Td>
                  <Operation code={entry.operation} />
                </Td>
                {showZone && (
                  <Td mono>
                    {entry.zone_id ? (
                      <Link href={`/zones/${encodeURIComponent(entry.zone_id)}`}>
                        {entry.zone_name ?? entry.zone_id}
                      </Link>
                    ) : entry.zone_name ? (
                      entry.zone_name
                    ) : (
                      <span className="font-sans text-fg-muted">Global</span>
                    )}
                  </Td>
                )}
                <Td>
                  <Outcome entry={entry} />
                </Td>
              </Tr>
              {isOpen && (
                <tr>
                  <td colSpan={columns} className="bg-surface-muted/50 px-3 py-3">
                    <DiffView diff={entry.diff} detail={entry.detail} />
                    {entry.client_ip && <p className="mt-2 text-xs text-fg-muted">Client address {entry.client_ip}</p>}
                  </td>
                </tr>
              )}
            </Fragment>
          );
        })}
      </TBody>
    </Table>
  );
}
