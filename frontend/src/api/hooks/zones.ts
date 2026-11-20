import { keepPreviousData, queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, zonePath } from "../client";
import type {
  ActionResult,
  CreateZoneRequest,
  HistoryPage,
  PatchRrSetsRequest,
  UpdateZoneRequest,
  Zone,
  ZoneExport,
  ZoneList,
  ZoneListParams,
} from "../types";

export const zoneKeys = {
  all: ["zones"] as const,
  list: (params: ZoneListParams) => ["zones", "list", params] as const,
  detail: (id: string) => ["zones", "detail", id] as const,
  history: (id: string) => ["zones", "history", id] as const,
};

export function zonesQuery(params: ZoneListParams) {
  return queryOptions({
    queryKey: zoneKeys.list(params),
    queryFn: ({ signal }) =>
      api.get<ZoneList>("/v1/zones", { q: params.q, page: params.page, per_page: params.per_page }, signal),
    placeholderData: keepPreviousData,
  });
}

export function useZones(params: ZoneListParams, enabled = true) {
  return useQuery({ ...zonesQuery(params), enabled });
}

export function zoneQuery(id: string) {
  return queryOptions({
    queryKey: zoneKeys.detail(id),
    queryFn: ({ signal }) => api.get<Zone>(zonePath(id), undefined, signal),
  });
}

export function useZone(id: string) {
  return useQuery(zoneQuery(id));
}

export function useZoneHistory(id: string, enabled = true) {
  return useQuery({
    queryKey: zoneKeys.history(id),
    queryFn: ({ signal }) => api.get<HistoryPage>(`${zonePath(id)}/history`, undefined, signal),
    enabled,
  });
}

export function useCreateZone() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateZoneRequest) => api.post<Zone>("/v1/zones", body),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: zoneKeys.all });
      void qc.invalidateQueries({ queryKey: ["dashboard"] });
    },
  });
}

export function useUpdateZone(id: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: UpdateZoneRequest) => api.put<void>(zonePath(id), body),
    onSuccess: () => qc.invalidateQueries({ queryKey: zoneKeys.all }),
  });
}

export function useDeleteZone(id: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.delete<void>(zonePath(id)),
    onSuccess: () => {
      qc.removeQueries({ queryKey: zoneKeys.detail(id) });
      void qc.invalidateQueries({ queryKey: zoneKeys.all });
      void qc.invalidateQueries({ queryKey: ["dashboard"] });
    },
  });
}

export function usePatchRrSets(id: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: PatchRrSetsRequest) => api.patch<void>(`${zonePath(id)}/rrsets`, body),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: zoneKeys.all });
      void qc.invalidateQueries({ queryKey: ["history"] });
    },
  });
}

export type ZoneAction = "notify" | "rectify" | "axfr-retrieve";

export function useZoneAction(id: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (action: ZoneAction) => api.post<ActionResult>(`${zonePath(id)}/${action}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: zoneKeys.detail(id) }),
  });
}

export function useZoneExport(id: string) {
  return useMutation({
    mutationFn: () => api.get<ZoneExport>(`${zonePath(id)}/export`),
  });
}
