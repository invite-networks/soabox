import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../client";
import type { Settings } from "../types";

const KEY = ["settings"] as const;

export function useSettings(enabled = true) {
  return useQuery({
    queryKey: KEY,
    queryFn: ({ signal }) => api.get<Settings>("/v1/settings", undefined, signal),
    enabled,
  });
}

export function useUpdateSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: Partial<Settings>) => api.put<Settings>("/v1/settings", body),
    onSuccess: (settings) => {
      qc.setQueryData(KEY, settings);
      void qc.invalidateQueries({ queryKey: ["me"] });
    },
  });
}
