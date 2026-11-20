import { createFileRoute } from "@tanstack/react-router";
import { Activity, Clock, FileStack, Globe, KeyRound, type LucideIcon, Plus, Server, ShieldCheck } from "lucide-react";
import { type ReactNode, useState } from "react";
import { useDashboard } from "../../api/hooks/dashboard";
import { CreateZoneDialog } from "../../components/create-zone-dialog";
import { HistoryTable } from "../../components/history-table";
import { Button, Card, CodeBlock, EmptyState, ErrorState, Link, LoadingState, PageHeader } from "../../components/ui";
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

function Stat({
  label,
  icon: Icon,
  children,
  hint,
}: {
  label: string;
  icon: LucideIcon;
  children: ReactNode;
  hint?: ReactNode;
}) {
  return (
    <div className="flex items-start gap-3 rounded-xl border border-border bg-surface px-4 py-4 shadow-[0_1px_2px_rgb(15_23_42/0.04)]">
      <span
        aria-hidden
        className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent-text"
      >
        <Icon className="size-[18px]" />
      </span>
      <div className="min-w-0">
        <p className="text-[11px] font-semibold tracking-wider text-fg-muted uppercase">{label}</p>
        <p className="mt-0.5 truncate text-xl font-semibold tracking-tight text-fg">{children}</p>
        {hint && <p className="mt-0.5 text-xs text-fg-muted">{hint}</p>}
      </div>
    </div>
  );
}

const ROLE_LABELS = { admin: "Administrator", operator: "Operator", user: "User" } as const;

function DashboardPage() {
  const me = useMe();
  const dashboard = useDashboard();
  const [createOpen, setCreateOpen] = useState(false);
  const name = me.user.display_name || me.user.username;
  const apiExample = `curl -H "X-API-Key: $SOABOX_KEY" \\\n  ${window.location.origin}/api/v1/servers/localhost/zones`;

  return (
    <>
      <PageHeader
        title="Dashboard"
        description={`Welcome back, ${name}.`}
        actions={
          can(me, "create_zones") && (
            <Button variant="primary" onPress={() => setCreateOpen(true)}>
              <Plus aria-hidden />
              Create zone
            </Button>
          )
        }
      />
      {dashboard.isPending ? (
        <LoadingState />
      ) : dashboard.isError ? (
        <ErrorState error={dashboard.error} onRetry={() => void dashboard.refetch()} />
      ) : (
        <div className="flex flex-col gap-6">
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
            <Stat label="Zones" icon={Globe} hint={<Link href="/zones">Browse zones</Link>}>
              {dashboard.data.zone_count.toLocaleString()}
            </Stat>
            <Stat
              label="PowerDNS"
              icon={Server}
              hint={dashboard.data.server ? "Authoritative server connected" : "Server did not respond"}
            >
              {dashboard.data.server?.version ?? "Unavailable"}
            </Stat>
            <Stat label="Server uptime" icon={Clock} hint="Since the last PowerDNS restart">
              {(() => {
                const up = uptimeSeconds(dashboard.data.server?.uptime ?? null);
                return up === null ? "Unknown" : formatDuration(up);
              })()}
            </Stat>
            <Stat
              label="Your access"
              icon={ShieldCheck}
              hint={me.permissions.all_zones ? "All zones" : "Zones assigned to you"}
            >
              {ROLE_LABELS[me.user.role]}
            </Stat>
          </div>

          <div className="grid grid-cols-1 gap-6 2xl:grid-cols-[minmax(0,1fr)_360px]">
            <div className="min-w-0">
              <Card
                title="Recent changes"
                flush
                {...(can(me, "view_history") ? { actions: <Link href="/history">View all history</Link> } : {})}
              >
                {dashboard.data.recent_history.length === 0 ? (
                  <EmptyState title="No changes yet">Changes to zones and records will appear here.</EmptyState>
                ) : (
                  <HistoryTable entries={dashboard.data.recent_history} label="Recent changes" />
                )}
              </Card>
            </div>
            <div className="grid min-w-0 grid-cols-1 content-start gap-6 lg:grid-cols-2 2xl:grid-cols-1">
              <Card title="Shortcuts">
                <ul className="flex flex-col gap-1">
                  {[
                    { href: "/zones", label: "Manage zones", icon: Globe, show: true },
                    { href: "/templates", label: "Zone templates", icon: FileStack, show: true },
                    { href: "/api-keys", label: "API keys", icon: KeyRound, show: can(me, "create_api_keys") },
                    { href: "/server", label: "Server statistics", icon: Activity, show: can(me, "view_server") },
                  ]
                    .filter((item) => item.show)
                    .map((item) => (
                      <li key={item.href}>
                        <Link href={item.href}>
                          <span className="flex items-center gap-2 py-1">
                            <item.icon aria-hidden className="size-4" />
                            {item.label}
                          </span>
                        </Link>
                      </li>
                    ))}
                </ul>
              </Card>
              <Card title="PowerDNS API">
                <div className="flex flex-col gap-3 text-sm text-fg-muted">
                  <p>
                    Existing PowerDNS tools work unchanged. Point them at SOAbox and use a SOAbox API key. Every change
                    is checked against your permissions and recorded in history.
                  </p>
                  <CodeBlock label="Example request">{apiExample}</CodeBlock>
                </div>
              </Card>
            </div>
          </div>
        </div>
      )}
      <CreateZoneDialog isOpen={createOpen} onOpenChange={setCreateOpen} />
    </>
  );
}
