import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useMemo, useState } from "react";
import { z } from "zod";
import { useServerConfig, useServerInfo, useServerStatistics } from "../../api/hooks/dashboard";
import type { ServerItem } from "../../api/types";
import { RequirePermission } from "../../components/forbidden";
import {
  Card,
  DetailList,
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

function ServerPage() {
  const search = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const tab = search.tab ?? "statistics";
  const info = useServerInfo();
  const statistics = useServerStatistics(tab === "statistics");
  const config = useServerConfig(tab === "configuration");

  return (
    <>
      <PageHeader title="Server" description="Read-only view of the PowerDNS server." />
      <div className="mb-4">
        <Card>
          {info.isPending ? (
            <LoadingState />
          ) : info.isError ? (
            <ErrorState error={info.error} onRetry={() => void info.refetch()} />
          ) : (
            <DetailList
              items={[
                { label: "Server", value: <Mono>{info.data.id}</Mono> },
                { label: "Daemon", value: info.data.daemon_type },
                { label: "Version", value: <Mono>{info.data.version}</Mono> },
              ]}
            />
          )}
        </Card>
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
