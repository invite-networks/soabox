import { useInfiniteQuery } from "@tanstack/react-query";
import { api } from "../client";
import type { HistoryFilters, HistoryPage } from "../types";

export const HISTORY_PAGE_SIZE = 50;

export const historyKeys = {
  list: (filters: HistoryFilters) => ["history", filters] as const,
};

/** Global history with cursor pagination through `before_id`. */
export function useHistory(filters: HistoryFilters) {
  return useInfiniteQuery({
    queryKey: historyKeys.list(filters),
    initialPageParam: undefined as number | undefined,
    queryFn: ({ pageParam, signal }) =>
      api.get<HistoryPage>(
        "/v1/history",
        {
          zone: filters.zone,
          actor: filters.actor,
          operation: filters.operation,
          since: filters.since,
          until: filters.until,
          before_id: pageParam,
          limit: HISTORY_PAGE_SIZE,
        },
        signal,
      ),
    getNextPageParam: (last) => {
      if (last.items.length < HISTORY_PAGE_SIZE) {
        return undefined;
      }
      return last.items[last.items.length - 1]?.id;
    },
  });
}
