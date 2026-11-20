import { useQuery } from "@tanstack/react-query";
import { api } from "../client";
import type { Dashboard, ServerInfo, ServerItem } from "../types";

export function useDashboard() {
  return useQuery({
    queryKey: ["dashboard"],
    queryFn: ({ signal }) => api.get<Dashboard>("/v1/dashboard", undefined, signal),
  });
}

export const serverKeys = {
  info: ["server", "info"] as const,
  statistics: ["server", "statistics"] as const,
  config: ["server", "config"] as const,
};

export function useServerInfo() {
  return useQuery({
    queryKey: serverKeys.info,
    queryFn: ({ signal }) => api.get<ServerInfo>("/v1/server", undefined, signal),
  });
}

export function useServerStatistics(enabled = true) {
  return useQuery({
    queryKey: serverKeys.statistics,
    queryFn: ({ signal }) => api.get<ServerItem[]>("/v1/server/statistics", undefined, signal),
    enabled,
  });
}

export function useServerConfig(enabled = true) {
  return useQuery({
    queryKey: serverKeys.config,
    queryFn: ({ signal }) => api.get<ServerItem[]>("/v1/server/config", undefined, signal),
    enabled,
  });
}
