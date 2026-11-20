import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { ChevronRight } from "lucide-react";
import { z } from "zod";
import { isApiError } from "../../../api/client";
import { useZone } from "../../../api/hooks/zones";
import {
  Alert,
  ErrorState,
  Link,
  LoadingState,
  Mono,
  PageHeader,
  Tab,
  TabList,
  TabPanel,
  Tabs,
} from "../../../components/ui";
import { DnssecTab } from "../../../components/zone/dnssec-tab";
import { HistoryTab } from "../../../components/zone/history-tab";
import { MetadataTab } from "../../../components/zone/metadata-tab";
import { RecordsTab } from "../../../components/zone/records-tab";
import { SettingsTab } from "../../../components/zone/settings-tab";
import { DnssecBadge, KIND_LABELS } from "../../../components/zone/zone-labels";
import { useMe } from "../../../lib/me-context";
import { can } from "../../../lib/permissions";

const TABS = ["records", "dnssec", "metadata", "history", "settings"] as const;
type ZoneTab = (typeof TABS)[number];

const searchSchema = z.object({
  tab: z.enum(TABS).optional().catch(undefined),
});

export const Route = createFileRoute("/_app/zones/$zoneId")({
  validateSearch: searchSchema,
  component: ZonePage,
});

function ZonePage() {
  const { zoneId } = Route.useParams();
  const search = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const me = useMe();
  const zone = useZone(zoneId);

  if (zone.isPending) {
    return <LoadingState label="Loading zone" />;
  }
  if (zone.isError) {
    if (isApiError(zone.error, 404)) {
      return (
        <>
          <PageHeader title="Zone not found" />
          <Alert tone="warning" title="This zone does not exist or you do not have access to it.">
            <Link href="/zones">Back to zones</Link>
          </Alert>
        </>
      );
    }
    return (
      <>
        <PageHeader title={zoneId} />
        <ErrorState error={zone.error} onRetry={() => void zone.refetch()} />
      </>
    );
  }

  const z = zone.data;
  const available: ZoneTab[] = TABS.filter((tab) => {
    if (tab === "dnssec" || tab === "metadata") {
      return can(me, "manage_dnssec");
    }
    if (tab === "history") {
      return can(me, "view_history");
    }
    return true;
  });
  const tab: ZoneTab = search.tab && available.includes(search.tab) ? search.tab : "records";
  const labels: Record<ZoneTab, string> = {
    records: "Records",
    dnssec: "DNSSEC",
    metadata: "Metadata",
    history: "History",
    settings: "Settings",
  };

  return (
    <>
      <PageHeader
        title={z.name}
        eyebrow={
          <span className="flex items-center gap-1.5">
            <Link href="/zones">Zones</Link>
            <ChevronRight aria-hidden className="size-3.5" />
            <span>{KIND_LABELS[z.kind] ?? z.kind} zone</span>
          </span>
        }
      />
      <dl className="mb-6 grid grid-cols-2 gap-px overflow-hidden rounded-xl border border-border bg-border shadow-[0_1px_2px_rgb(15_23_42/0.04)] lg:grid-cols-5">
        {[
          { label: "Kind", value: KIND_LABELS[z.kind] ?? z.kind },
          { label: "Serial", value: <Mono>{z.serial}</Mono> },
          { label: "DNSSEC", value: <DnssecBadge enabled={z.dnssec} /> },
          {
            label: "Records",
            value: `${z.rrsets.reduce((n, r) => n + r.records.length, 0)} in ${z.rrsets.length} RRsets`,
          },
          { label: "Account", value: z.account?.name ?? <span className="text-fg-muted">Unassigned</span> },
        ].map((item) => (
          <div key={item.label} className="flex flex-col gap-1 bg-surface px-4 py-3 last:max-lg:col-span-2">
            <dt className="text-[11px] font-semibold tracking-wider text-fg-muted uppercase">{item.label}</dt>
            <dd className="text-sm font-medium text-fg">{item.value}</dd>
          </div>
        ))}
      </dl>
      <Tabs
        selectedKey={tab}
        onSelectionChange={(key) =>
          void navigate({ search: { tab: key === "records" ? undefined : (key as ZoneTab) }, replace: true })
        }
      >
        <TabList label="Zone sections">
          {available.map((t) => (
            <Tab key={t} id={t}>
              {labels[t]}
            </Tab>
          ))}
        </TabList>
        <TabPanel id="records">
          <RecordsTab zone={z} />
        </TabPanel>
        {available.includes("dnssec") && (
          <TabPanel id="dnssec">
            <DnssecTab zone={z} />
          </TabPanel>
        )}
        {available.includes("metadata") && (
          <TabPanel id="metadata">
            <MetadataTab zone={z} />
          </TabPanel>
        )}
        {available.includes("history") && (
          <TabPanel id="history">
            <HistoryTab zone={z} />
          </TabPanel>
        )}
        <TabPanel id="settings">
          <SettingsTab zone={z} />
        </TabPanel>
      </Tabs>
    </>
  );
}
