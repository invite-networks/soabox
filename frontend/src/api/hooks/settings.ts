import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../client";
import type { OidcSettings, OidcSettingsUpdate, Settings } from "../types";

const KEY = ["settings"] as const;
const OIDC_KEY = ["oidc-settings"] as const;

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

export function useOidcSettings() {
  return useQuery({
    queryKey: OIDC_KEY,
    queryFn: ({ signal }) => api.get<OidcSettings>("/v1/settings/oidc", undefined, signal),
  });
}

export function useUpdateOidcSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: OidcSettingsUpdate) => api.put<OidcSettings>("/v1/settings/oidc", body),
    onSuccess: (oidc) => {
      qc.setQueryData(OIDC_KEY, oidc);
      void qc.invalidateQueries({ queryKey: ["auth-config"] });
    },
  });
}
