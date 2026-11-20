import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import { useEffect, useState } from "react";
import { z } from "zod";
import { useZones } from "../../../api/hooks/zones";
import { CreateZoneDialog } from "../../../components/create-zone-dialog";
import {
  Button,
  Card,
  EmptyState,
  ErrorState,
  Link,
  LoadingState,
  PageHeader,
  Pagination,
  RowHeader,
  SearchField,
  Table,
  TBody,
  Td,
  THead,
  Th,
  Tr,
} from "../../../components/ui";
import { DnssecBadge, KIND_LABELS } from "../../../components/zone/zone-labels";
import { useMe } from "../../../lib/me-context";
import { can } from "../../../lib/permissions";

const PER_PAGE = 50;

const searchSchema = z.object({
  q: z.string().optional().catch(undefined),
  page: z.number().int().min(1).optional().catch(undefined),
});

export const Route = createFileRoute("/_app/zones/")({
  validateSearch: searchSchema,
  component: ZonesPage,
});

function ZonesPage() {
  const me = useMe();
  const search = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const page = search.page ?? 1;
  const [input, setInput] = useState(search.q ?? "");
  const [createOpen, setCreateOpen] = useState(false);
  const zones = useZones({ ...(search.q ? { q: search.q } : {}), page, per_page: PER_PAGE });

  useEffect(() => {
    const q = input.trim();
    if (q === (search.q ?? "")) {
      return;
    }
    const timer = window.setTimeout(() => {
      void navigate({ search: { ...(q ? { q } : {}) }, replace: true });
    }, 250);
    return () => window.clearTimeout(timer);
  }, [input, search.q, navigate]);

  const pageCount = zones.data ? Math.max(1, Math.ceil(zones.data.total / PER_PAGE)) : 1;

  return (
    <>
      <PageHeader
        title="Zones"
        actions={
          can(me, "create_zones") && (
            <Button variant="primary" onPress={() => setCreateOpen(true)}>
              <Plus aria-hidden />
              Create zone
            </Button>
          )
        }
      />
      <Card flush>
        <div className="border-b border-border p-3">
          <div className="max-w-sm">
            <SearchField label="Search zones" hideLabel placeholder="Search zones" value={input} onChange={setInput} />
          </div>
        </div>
        {zones.isPending ? (
          <LoadingState label="Loading zones" />
        ) : zones.isError ? (
          <ErrorState error={zones.error} onRetry={() => void zones.refetch()} />
        ) : zones.data.items.length === 0 ? (
          <EmptyState
            title={search.q ? "No zones match your search" : "No zones yet"}
            action={
              !search.q &&
              can(me, "create_zones") && (
                <Button variant="primary" onPress={() => setCreateOpen(true)}>
                  Create zone
                </Button>
              )
            }
          >
            {search.q ? "Try a different name." : "Zones you can manage will appear here."}
          </EmptyState>
        ) : (
          <>
            <Table label="Zones">
              <THead>
                <tr>
                  <Th>Name</Th>
                  <Th>Kind</Th>
                  <Th align="right">Serial</Th>
                  <Th>DNSSEC</Th>
                  <Th>Account</Th>
                </tr>
              </THead>
              <TBody>
                {zones.data.items.map((zone) => (
                  <Tr key={zone.id}>
                    <RowHeader mono>
                      <Link href={`/zones/${encodeURIComponent(zone.id)}`}>{zone.name}</Link>
                    </RowHeader>
                    <Td>{KIND_LABELS[zone.kind] ?? zone.kind}</Td>
                    <Td align="right" mono>
                      {zone.serial}
                    </Td>
                    <Td>
                      <DnssecBadge enabled={zone.dnssec} />
                    </Td>
                    <Td>{zone.account?.name ?? <span className="text-fg-muted">None</span>}</Td>
                  </Tr>
                ))}
              </TBody>
            </Table>
            <Pagination
              page={page}
              pageCount={pageCount}
              total={zones.data.total}
              noun={zones.data.total === 1 ? "zone" : "zones"}
              onPageChange={(next) => void navigate({ search: (prev) => ({ ...prev, page: next }) })}
            />
          </>
        )}
      </Card>
      <CreateZoneDialog isOpen={createOpen} onOpenChange={setCreateOpen} />
    </>
  );
}
