import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/client";
import type { User } from "../api/types";

export function useAuth() {
  const qc = useQueryClient();

  const { data: user, isLoading, error } = useQuery<User>({
    queryKey: ["auth", "me"],
    queryFn: () => api.get("/auth/me"),
    retry: false,
  });

  const logout = useMutation({
    mutationFn: () => api.post("/auth/logout"),
    onSuccess: () => {
      qc.clear();
      window.location.href = "/login";
    },
  });

  return { user, isLoading, error, logout: logout.mutate, isAdmin: user?.role === "admin" };
}
