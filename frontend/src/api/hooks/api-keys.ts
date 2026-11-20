import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../client";
import type { ApiKey, ApiKeyInput, CreatedApiKey } from "../types";

const KEY = ["api-keys"] as const;

export function useApiKeys() {
  return useQuery({
    queryKey: KEY,
    queryFn: ({ signal }) => api.get<ApiKey[]>("/v1/api-keys", undefined, signal),
  });
}

export function useCreateApiKey() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: ApiKeyInput) => api.post<CreatedApiKey>("/v1/api-keys", body),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export function useUpdateApiKey() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, body }: { id: number; body: ApiKeyInput }) => api.put<ApiKey>(`/v1/api-keys/${id}`, body),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export type ApiKeyAction = "enable" | "disable" | "delete";

export function useApiKeyAction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, action }: { id: number; action: ApiKeyAction }) =>
      action === "delete" ? api.delete<void>(`/v1/api-keys/${id}`) : api.post<void>(`/v1/api-keys/${id}/${action}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}
