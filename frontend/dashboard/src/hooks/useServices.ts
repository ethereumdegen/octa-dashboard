import { useQuery } from "@tanstack/react-query";
import { api } from "../api/client";
import type { ServiceInfo } from "../api/types";

export function useServices() {
  return useQuery<ServiceInfo[]>({
    queryKey: ["services"],
    queryFn: () => api.get("/services"),
  });
}
