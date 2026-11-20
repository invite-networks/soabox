import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, zonePath } from "../client";
import type { CreateCryptoKeyRequest, CryptoKey, Metadata, SetDnssecRequest, UpdateCryptoKeyRequest } from "../types";
import { zoneKeys } from "./zones";

export const dnssecKeys = {
  cryptokeys: (id: string) => ["zones", "cryptokeys", id] as const,
  metadata: (id: string) => ["zones", "metadata", id] as const,
};

export function useCryptoKeys(zoneId: string, enabled = true) {
  return useQuery({
    queryKey: dnssecKeys.cryptokeys(zoneId),
    queryFn: ({ signal }) => api.get<CryptoKey[]>(`${zonePath(zoneId)}/cryptokeys`, undefined, signal),
    enabled,
  });
}

function useInvalidateZone(zoneId: string) {
  const qc = useQueryClient();
  return () => {
    void qc.invalidateQueries({ queryKey: dnssecKeys.cryptokeys(zoneId) });
    void qc.invalidateQueries({ queryKey: zoneKeys.detail(zoneId) });
    void qc.invalidateQueries({ queryKey: ["zones", "list"] });
  };
}

export function useSetDnssec(zoneId: string) {
  const invalidate = useInvalidateZone(zoneId);
  return useMutation({
    mutationFn: (body: SetDnssecRequest) => api.post<void>(`${zonePath(zoneId)}/dnssec`, body),
    onSuccess: invalidate,
  });
}

export function useCreateCryptoKey(zoneId: string) {
  const invalidate = useInvalidateZone(zoneId);
  return useMutation({
    mutationFn: (body: CreateCryptoKeyRequest) => api.post<CryptoKey>(`${zonePath(zoneId)}/cryptokeys`, body),
    onSuccess: invalidate,
  });
}

export function useUpdateCryptoKey(zoneId: string) {
  const invalidate = useInvalidateZone(zoneId);
  return useMutation({
    mutationFn: ({ keyId, body }: { keyId: number; body: UpdateCryptoKeyRequest }) =>
      api.put<void>(`${zonePath(zoneId)}/cryptokeys/${keyId}`, body),
    onSuccess: invalidate,
  });
}

export function useDeleteCryptoKey(zoneId: string) {
  const invalidate = useInvalidateZone(zoneId);
  return useMutation({
    mutationFn: (keyId: number) => api.delete<void>(`${zonePath(zoneId)}/cryptokeys/${keyId}`),
    onSuccess: invalidate,
  });
}

export function useMetadata(zoneId: string, enabled = true) {
  return useQuery({
    queryKey: dnssecKeys.metadata(zoneId),
    queryFn: ({ signal }) => api.get<Metadata[]>(`${zonePath(zoneId)}/metadata`, undefined, signal),
    enabled,
  });
}

export function usePutMetadata(zoneId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ kind, metadata }: Metadata) =>
      api.put<void>(`${zonePath(zoneId)}/metadata/${encodeURIComponent(kind)}`, { metadata }),
    onSuccess: () => qc.invalidateQueries({ queryKey: dnssecKeys.metadata(zoneId) }),
  });
}

export function useDeleteMetadata(zoneId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (kind: string) => api.delete<void>(`${zonePath(zoneId)}/metadata/${encodeURIComponent(kind)}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: dnssecKeys.metadata(zoneId) }),
  });
}
