import { useQuery } from "@tanstack/react-query";
import { api } from "../api/client";
import type { AnalyticsSummary, ChartData } from "../api/types";

export function useAnalyticsSummary() {
  return useQuery<AnalyticsSummary>({
    queryKey: ["analytics", "summary"],
    queryFn: () => api.get("/analytics/summary"),
  });
}

export function useAnalyticsChart(metric: string, days = 7) {
  return useQuery<ChartData>({
    queryKey: ["analytics", "chart", metric, days],
    queryFn: () => api.get(`/analytics/chart/${metric}?days=${days}`),
  });
}
