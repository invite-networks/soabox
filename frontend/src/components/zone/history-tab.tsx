import { useZoneHistory } from "../../api/hooks/zones";
import type { Zone } from "../../api/types";
import { HistoryTable } from "../history-table";
import { Card, EmptyState, ErrorState, LoadingState } from "../ui";

export function HistoryTab({ zone }: { zone: Zone }) {
  const history = useZoneHistory(zone.id);
  return (
    <Card flush>
      {history.isPending ? (
        <LoadingState label="Loading history" />
      ) : history.isError ? (
        <ErrorState error={history.error} onRetry={() => void history.refetch()} />
      ) : history.data.items.length === 0 ? (
        <EmptyState title="No history for this zone yet" />
      ) : (
        <HistoryTable entries={history.data.items} label="Zone history" showZone={false} />
      )}
    </Card>
  );
}
