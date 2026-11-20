import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../client";
import type { CreateTsigKeyRequest, TsigKey } from "../types";

const KEY = ["tsigkeys"] as const;

export function useTsigKeys() {
  return useQuery({
    queryKey: KEY,
    queryFn: ({ signal }) => api.get<TsigKey[]>("/v1/tsigkeys", undefined, signal),
  });
}

export function useTsigKey(id: string | null) {
  return useQuery({
    queryKey: [...KEY, id],
    queryFn: ({ signal }) => api.get<TsigKey>(`/v1/tsigkeys/${encodeURIComponent(id ?? "")}`, undefined, signal),
    enabled: id !== null,
    staleTime: 0,
    gcTime: 0,
  });
}

export function useCreateTsigKey() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateTsigKeyRequest) => api.post<TsigKey>("/v1/tsigkeys", body),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export function useDeleteTsigKey() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.delete<void>(`/v1/tsigkeys/${encodeURIComponent(id)}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}
