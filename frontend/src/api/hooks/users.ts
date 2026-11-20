import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../client";
import type { CreateUserRequest, UpdateUserRequest, User } from "../types";

const KEY = ["users"] as const;

export function useUsers(enabled = true) {
  return useQuery({
    queryKey: KEY,
    queryFn: ({ signal }) => api.get<User[]>("/v1/users", undefined, signal),
    enabled,
  });
}

export function useCreateUser() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateUserRequest) => api.post<User>("/v1/users", body),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export function useUpdateUser() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, body }: { id: number; body: UpdateUserRequest }) => api.put<User>(`/v1/users/${id}`, body),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: KEY });
      void qc.invalidateQueries({ queryKey: ["accounts"] });
    },
  });
}

export type UserAction = "revoke-sessions" | "reset-totp" | "delete";

export function useUserAction() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, action }: { id: number; action: UserAction }) => {
      switch (action) {
        case "revoke-sessions":
          return api.post<void>(`/v1/users/${id}/revoke-sessions`);
        case "reset-totp":
          return api.delete<void>(`/v1/users/${id}/totp`);
        case "delete":
          return api.delete<void>(`/v1/users/${id}`);
      }
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export function useResetUserPassword() {
  return useMutation({
    mutationFn: ({ id, password }: { id: number; password: string }) =>
      api.post<void>(`/v1/users/${id}/password`, { password }),
  });
}
