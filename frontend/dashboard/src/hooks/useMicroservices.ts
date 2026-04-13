import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/client";
import type { Microservice } from "../api/types";

export function useMicroservices() {
  return useQuery<Microservice[]>({
    queryKey: ["microservices"],
    queryFn: () => api.get("/microservices"),
  });
}

export function useMicroserviceMutations() {
  const qc = useQueryClient();

  const toggle = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      api.put<Microservice>(`/microservices/${id}`, { enabled }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["microservices"] });
      qc.invalidateQueries({ queryKey: ["services"] });
    },
  });

  return { toggle };
}
