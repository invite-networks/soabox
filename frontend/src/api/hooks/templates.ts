import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../client";
import type { Template, TemplateInput } from "../types";

const KEY = ["templates"] as const;

export function useTemplates() {
  return useQuery({
    queryKey: KEY,
    queryFn: ({ signal }) => api.get<Template[]>("/v1/templates", undefined, signal),
  });
}

export function useSaveTemplate() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, body }: { id: number | null; body: TemplateInput }) =>
      id === null ? api.post<Template>("/v1/templates", body) : api.put<Template>(`/v1/templates/${id}`, body),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}

export function useDeleteTemplate() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => api.delete<void>(`/v1/templates/${id}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: KEY }),
  });
}
