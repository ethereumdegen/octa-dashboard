import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/client";
import type { TeamMember } from "../api/types";

export function useTeam() {
  const qc = useQueryClient();

  const query = useQuery<TeamMember[]>({
    queryKey: ["team"],
    queryFn: () => api.get("/team"),
  });

  const addMember = useMutation({
    mutationFn: (body: { email: string; role?: string }) => api.post<TeamMember>("/team", body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["team"] }),
  });

  const removeMember = useMutation({
    mutationFn: (id: string) => api.delete(`/team/${id}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["team"] }),
  });

  return { ...query, addMember, removeMember };
}
