import { createFileRoute } from "@tanstack/react-router";
import { useDashboard } from "../../api/hooks/dashboard";
import { HistoryTable } from "../../components/history-table";
import { Card, EmptyState, ErrorState, Link, LoadingState, Mono, PageHeader } from "../../components/ui";
import { formatDuration } from "../../lib/format";
import { useMe } from "../../lib/me-context";
import { can } from "../../lib/permissions";

export const Route = createFileRoute("/_app/")({
  component: DashboardPage,
});

/** PowerDNS reports uptime as a string of seconds. */
function uptimeSeconds(value: string | null): number | null {
  if (value === null || !/^\d+$/.test(value.trim())) {
    return null;
  }
  return Number(value.trim());
}

function Stat({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="rounded-lg border border-border bg-surface px-4 py-3">
      <p className="text-xs text-fg-muted">{label}</p>
      <p className="mt-1 text-lg font-semibold text-fg">{children}</p>
    </div>
  );
}

function DashboardPage() {
  const me = useMe();
  const dashboard = useDashboard();
  return (
    <>
      <PageHeader title="Dashboard" />
      {dashboard.isPending ? (
        <LoadingState />
      ) : dashboard.isError ? (
        <ErrorState error={dashboard.error} onRetry={() => void dashboard.refetch()} />
      ) : (
        <div className="flex flex-col gap-4">
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
            <Stat label="Zones">
              <Link href="/zones">{dashboard.data.zone_count}</Link>
            </Stat>
            {dashboard.data.server && (
              <>
                <Stat label="PowerDNS version">
                  <Mono>{dashboard.data.server.version}</Mono>
                </Stat>
                {uptimeSeconds(dashboard.data.server.uptime) !== null && (
                  <Stat label="Server uptime">{formatDuration(uptimeSeconds(dashboard.data.server.uptime) ?? 0)}</Stat>
                )}
              </>
            )}
          </div>
          <Card
            title="Recent changes"
            flush
            {...(can(me, "view_history") ? { actions: <Link href="/history">All history</Link> } : {})}
          >
            {dashboard.data.recent_history.length === 0 ? (
              <EmptyState title="No changes yet">Changes to zones and records will appear here.</EmptyState>
            ) : (
              <HistoryTable entries={dashboard.data.recent_history} label="Recent changes" />
            )}
          </Card>
        </div>
      )}
    </>
  );
}
