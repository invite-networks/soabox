import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useMemo, useState } from "react";
import { z } from "zod";
import { useServerConfig, useServerInfo, useServerStatistics } from "../../api/hooks/dashboard";
import type { ServerItem } from "../../api/types";
import { RequirePermission } from "../../components/forbidden";
import {
  Card,
  EmptyState,
  ErrorState,
  LoadingState,
  Mono,
  PageHeader,
  RowHeader,
  SearchField,
  Tab,
  TabList,
  Table,
  TabPanel,
  Tabs,
  TBody,
  Td,
  THead,
  Th,
  Tr,
} from "../../components/ui";
import { formatDuration } from "../../lib/format";

const TABS = ["statistics", "configuration"] as const;

export const Route = createFileRoute("/_app/server")({
  validateSearch: z.object({ tab: z.enum(TABS).optional().catch(undefined) }),
  component: () => (
    <RequirePermission permission="view_server" title="Server">
      <ServerPage />
    </RequirePermission>
  ),
});

function valueText(item: ServerItem): string {
  if (typeof item.value === "string") {
    return item.value;
  }
  return item.value.map((v) => `${v.name}=${v.value}`).join(", ");
}

function ItemTable({ label, query }: { label: string; query: ReturnType<typeof useServerStatistics> }) {
  const [filter, setFilter] = useState("");
  const items = useMemo(() => {
    const q = filter.trim().toLowerCase();
    const all = query.data ?? [];
    return q ? all.filter((i) => i.name.toLowerCase().includes(q)) : all;
  }, [query.data, filter]);
  return (
    <Card flush>
      <div className="border-b border-border p-3">
        <div className="max-w-sm">
          <SearchField
            label={`Filter ${label.toLowerCase()}`}
            hideLabel
            placeholder="Filter by name"
            value={filter}
            onChange={setFilter}
          />
        </div>
      </div>
      {query.isPending ? (
        <LoadingState />
      ) : query.isError ? (
        <ErrorState error={query.error} onRetry={() => void query.refetch()} />
      ) : items.length === 0 ? (
        <EmptyState title="Nothing matches the filter" />
      ) : (
        <Table label={label}>
          <THead>
            <tr>
              <Th>Name</Th>
              <Th>Value</Th>
            </tr>
          </THead>
          <TBody>
            {items.map((item) => (
              <Tr key={item.name}>
                <RowHeader mono>{item.name}</RowHeader>
                <Td mono wrap>
                  {valueText(item)}
                </Td>
              </Tr>
            ))}
          </TBody>
        </Table>
      )}
    </Card>
  );
}

function statNumber(items: readonly ServerItem[] | undefined, name: string): number | null {
  const item = items?.find((i) => i.name === name);
  if (!item || typeof item.value !== "string" || !/^\d+$/.test(item.value)) {
    return null;
  }
  return Number(item.value);
}

/** Headline numbers drawn from the PowerDNS statistics. */
function Metrics({ items }: { items: readonly ServerItem[] | undefined }) {
  const n = (name: string) => statNumber(items, name);
  const sum = (...names: string[]) => {
    const values = names.map(n);
    return values.every((v) => v === null) ? null : values.reduce<number>((a, v) => a + (v ?? 0), 0);
  };
  const queries = sum("udp-queries", "tcp-queries");
  const hit = n("packetcache-hit");
  const miss = n("packetcache-miss");
  const hitRate =
    hit !== null && miss !== null && hit + miss > 0 ? `${((hit / (hit + miss)) * 100).toFixed(1)}%` : "No traffic";
  const uptime = n("uptime");
  const latency = n("latency");
  const metrics = [
    { label: "Uptime", value: uptime === null ? "Unknown" : formatDuration(uptime) },
    { label: "Queries answered", value: queries === null ? "Unknown" : queries.toLocaleString() },
    { label: "Packet cache hit rate", value: hitRate },
    { label: "Average latency", value: latency === null ? "Unknown" : `${latency.toLocaleString()} µs` },
    { label: "Zones in cache", value: n("zone-cache-size")?.toLocaleString() ?? "Unknown" },
  ];
  return (
    <dl className="grid grid-cols-2 gap-px overflow-hidden rounded-xl border border-border bg-border lg:grid-cols-5">
      {metrics.map((m) => (
        <div key={m.label} className="flex flex-col gap-1 bg-surface px-4 py-3 last:max-lg:col-span-2">
          <dt className="text-[11px] font-semibold tracking-wider text-fg-muted uppercase">{m.label}</dt>
          <dd className="text-lg font-semibold tracking-tight text-fg">{m.value}</dd>
        </div>
      ))}
    </dl>
  );
}

function ServerPage() {
  const search = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const tab = search.tab ?? "statistics";
  const info = useServerInfo();
  const statistics = useServerStatistics();
  const config = useServerConfig(tab === "configuration");

  return (
    <>
      <PageHeader
        title="Server"
        description={
          info.data ? (
            <span>
              PowerDNS {info.data.daemon_type} <Mono>{info.data.version}</Mono> on server <Mono>{info.data.id}</Mono>.
              Read only.
            </span>
          ) : (
            "Read-only view of the PowerDNS server."
          )
        }
      />
      {info.isError && (
        <div className="mb-4">
          <ErrorState error={info.error} onRetry={() => void info.refetch()} />
        </div>
      )}
      <div className="mb-6">
        <Metrics items={statistics.data} />
      </div>
      <Tabs
        selectedKey={tab}
        onSelectionChange={(key) =>
          void navigate({ search: { tab: key === "configuration" ? "configuration" : undefined }, replace: true })
        }
      >
        <TabList label="Server sections">
          <Tab id="statistics">Statistics</Tab>
          <Tab id="configuration">Configuration</Tab>
        </TabList>
        <TabPanel id="statistics">
          <ItemTable label="Statistics" query={statistics} />
        </TabPanel>
        <TabPanel id="configuration">
          <ItemTable label="Configuration" query={config} />
        </TabPanel>
      </Tabs>
    </>
  );
}
