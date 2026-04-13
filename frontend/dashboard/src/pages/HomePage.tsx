import PageHeader from "../components/PageHeader";
import StatCard from "../components/StatCard";
import ChartCard from "../components/ChartCard";
import { useAnalyticsSummary, useAnalyticsChart } from "../hooks/useAnalytics";

export default function HomePage() {
  const { data: summary } = useAnalyticsSummary();
  const { data: loginChart } = useAnalyticsChart("login", 14);
  const { data: pageviewChart } = useAnalyticsChart("pageview", 14);

  return (
    <div>
      <PageHeader title="Home" />

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-6 mb-8">
        <StatCard label="Total Users" value={summary?.total_users ?? 0} />
        <StatCard label="Active Agents" value={summary?.total_agents ?? 0} />
        <StatCard label="Documents" value={summary?.total_documents ?? 0} />
        <StatCard label="Logins Today" value={summary?.logins_today ?? 0} />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <ChartCard title="Logins (14 days)" data={loginChart?.data ?? []} />
        <ChartCard title="Page Views (14 days)" data={pageviewChart?.data ?? []} />
      </div>
    </div>
  );
}
