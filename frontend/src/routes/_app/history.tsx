import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { z } from "zod";
import { useHistory } from "../../api/hooks/history";
import { RequirePermission } from "../../components/forbidden";
import { HistoryTable } from "../../components/history-table";
import { Button, Card, EmptyState, ErrorState, Form, LoadingState, PageHeader, TextField } from "../../components/ui";
import { dateToUnix } from "../../lib/format";

const dateString = z
  .string()
  .regex(/^\d{4}-\d{2}-\d{2}$/)
  .optional()
  .catch(undefined);

const searchSchema = z.object({
  zone: z.string().optional().catch(undefined),
  actor: z.string().optional().catch(undefined),
  operation: z.string().optional().catch(undefined),
  since: dateString,
  until: dateString,
});
type HistorySearch = z.infer<typeof searchSchema>;

export const Route = createFileRoute("/_app/history")({
  validateSearch: searchSchema,
  component: () => (
    <RequirePermission permission="view_history" title="History">
      <HistoryPage />
    </RequirePermission>
  ),
});

function clean(values: HistorySearch): HistorySearch {
  const out: HistorySearch = {};
  for (const key of ["zone", "actor", "operation", "since", "until"] as const) {
    const v = values[key]?.trim();
    if (v) {
      out[key] = v;
    }
  }
  return out;
}

function HistoryPage() {
  const search = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const [draft, setDraft] = useState<HistorySearch>(search);
  const since = dateToUnix(search.since);
  const until = dateToUnix(search.until, true);
  const history = useHistory({
    ...(search.zone ? { zone: search.zone } : {}),
    ...(search.actor ? { actor: search.actor } : {}),
    ...(search.operation ? { operation: search.operation } : {}),
    ...(since !== undefined ? { since } : {}),
    ...(until !== undefined ? { until } : {}),
  });
  const entries = history.data?.pages.flatMap((p) => p.items) ?? [];
  const field = (key: keyof HistorySearch) => ({
    value: draft[key] ?? "",
    onChange: (value: string) => setDraft((d) => ({ ...d, [key]: value })),
  });
  const hasFilters = Object.keys(clean(search)).length > 0;

  return (
    <>
      <PageHeader title="History" />
      <Card flush>
        <div className="border-b border-border p-3">
          <Form label="Filter history" onSubmit={() => void navigate({ search: clean(draft) })}>
            <div className="grid grid-cols-1 items-end gap-3 sm:grid-cols-3 lg:grid-cols-6">
              <TextField label="Zone" placeholder="example.com." mono {...field("zone")} />
              <TextField label="Actor" {...field("actor")} />
              <TextField label="Operation" mono {...field("operation")} />
              <TextField label="From" type="date" {...field("since")} />
              <TextField label="To" type="date" {...field("until")} />
              <div className="flex gap-2">
                <Button type="submit" variant="primary">
                  Apply filters
                </Button>
                {hasFilters && (
                  <Button
                    onPress={() => {
                      setDraft({});
                      void navigate({ search: {} });
                    }}
                  >
                    Clear
                  </Button>
                )}
              </div>
            </div>
          </Form>
        </div>
        {history.isPending ? (
          <LoadingState label="Loading history" />
        ) : history.isError ? (
          <ErrorState error={history.error} onRetry={() => void history.refetch()} />
        ) : entries.length === 0 ? (
          <EmptyState title={hasFilters ? "No history matches these filters" : "No history yet"} />
        ) : (
          <>
            <HistoryTable entries={entries} label="History" />
            {history.hasNextPage && (
              <div className="flex justify-center border-t border-border p-3">
                <Button isPending={history.isFetchingNextPage} onPress={() => void history.fetchNextPage()}>
                  Load older entries
                </Button>
              </div>
            )}
          </>
        )}
      </Card>
    </>
  );
}
