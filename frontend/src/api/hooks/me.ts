import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, setCsrfToken } from "../client";
import type { AuthConfig, ChangePasswordRequest, LoginRequest, Me, TotpSetup, UpdateMeRequest } from "../types";

export const meKeys = {
  me: ["me"] as const,
  authConfig: ["auth-config"] as const,
};

async function fetchMe(signal?: AbortSignal): Promise<Me> {
  const me = await api.get<Me>("/v1/me", undefined, signal);
  setCsrfToken(me.csrf_token);
  return me;
}

/** Query options for the current user. */
export const meQuery = queryOptions({
  queryKey: meKeys.me,
  queryFn: ({ signal }) => fetchMe(signal),
  staleTime: 60_000,
});

/** The current user. Routes under the app layout guarantee this is loaded. */
export function useMeQuery() {
  return useQuery(meQuery);
}

export const authConfigQuery = queryOptions({
  queryKey: meKeys.authConfig,
  queryFn: ({ signal }) => api.get<AuthConfig>("/auth/config", undefined, signal),
  staleTime: 300_000,
});

export function useAuthConfig() {
  return useQuery(authConfigQuery);
}

export function useLogin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: LoginRequest) => api.post<Me>("/auth/login", body),
    onSuccess: (me) => {
      setCsrfToken(me.csrf_token);
      qc.setQueryData(meKeys.me, me);
    },
  });
}

export function useLogout() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.post<void>("/auth/logout"),
    onSettled: () => {
      setCsrfToken(null);
      qc.clear();
    },
  });
}

export function useUpdateMe() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: UpdateMeRequest) => api.put<Me>("/v1/me", body),
    onSuccess: (me) => {
      setCsrfToken(me.csrf_token);
      qc.setQueryData(meKeys.me, me);
    },
  });
}

export function useChangePassword() {
  return useMutation({
    mutationFn: (body: ChangePasswordRequest) => api.put<void>("/v1/me/password", body),
  });
}

export function useTotpSetup() {
  return useMutation({
    mutationFn: () => api.post<TotpSetup>("/v1/me/totp/setup"),
  });
}

export function useTotpEnable() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (code: string) => api.post<void>("/v1/me/totp/enable", { code }),
    onSuccess: () => qc.invalidateQueries({ queryKey: meKeys.me }),
  });
}

export function useTotpDisable() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: { code: string; password: string }) => api.delete<void>("/v1/me/totp", input),
    onSuccess: () => qc.invalidateQueries({ queryKey: meKeys.me }),
  });
}
