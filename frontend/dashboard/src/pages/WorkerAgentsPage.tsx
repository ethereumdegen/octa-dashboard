import { useState, useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { Bot, ExternalLink, Heart, Clock, Cpu, Globe, Activity, Zap, ArrowLeft } from "lucide-react";
import { Link } from "react-router";
import PageHeader from "../components/PageHeader";
import { useAgents } from "../hooks/useAgents";
import { useMicroservices } from "../hooks/useMicroservices";
import { api } from "../api/client";
import type { Agent, Microservice } from "../api/types";

function useAgentHealth(agentId: string | undefined) {
  return useQuery<{ status: string; uptime_seconds: number }>({
    queryKey: ["agent-health", agentId],
    queryFn: () => api.get(`/agents/${agentId}/health`),
    enabled: !!agentId,
    refetchInterval: 10_000,
  });
}

function fmtUptime(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h < 24) return `${h}h ${m}m`;
  const d = Math.floor(h / 24);
  return `${d}d ${h % 24}h`;
}

function AgentDetail({
  agent,
  microservice,
}: {
  agent: Agent;
  microservice: Microservice | undefined;
}) {
  const { data: health, isLoading, isError } = useAgentHealth(agent.id);

  const statusColor =
    agent.status === "healthy"
      ? "bg-green-100 text-green-700"
      : "bg-red-100 text-red-700";

  const healthColor =
    health?.status === "healthy"
      ? "text-green-500"
      : isError
        ? "text-red-500"
        : "text-gray-400";

  return (
    <div className="p-6 space-y-6 h-full overflow-auto">
      {/* Header + link */}
      <div className="flex items-start justify-between">
        <div>
          <h2 className="text-lg font-semibold text-gray-900">{agent.name}</h2>
          <p className="text-sm text-gray-500 mt-0.5">{agent.manifest.description}</p>
        </div>
        {microservice && (
          <Link
            to={microservice.nav_path}
            className="flex items-center gap-1.5 px-4 py-2 bg-gray-900 text-white text-sm font-medium rounded-xl hover:bg-gray-800 transition-colors shrink-0"
          >
            Open App
            <ExternalLink size={14} />
          </Link>
        )}
      </div>

      {/* Status cards */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 sm:gap-4">
        <div className="bg-gray-50 rounded-xl p-4">
          <div className="flex items-center gap-2 text-gray-500 mb-2">
            <Heart size={14} className={healthColor} />
            <span className="text-xs font-medium">Status</span>
          </div>
          <span className={`inline-flex px-2 py-0.5 rounded-full text-xs font-medium ${statusColor}`}>
            {agent.status}
          </span>
        </div>

        <div className="bg-gray-50 rounded-xl p-4">
          <div className="flex items-center gap-2 text-gray-500 mb-2">
            <Clock size={14} />
            <span className="text-xs font-medium">Uptime</span>
          </div>
          <p className="text-sm font-medium text-gray-900">
            {isLoading ? "..." : health ? fmtUptime(health.uptime_seconds) : "-"}
          </p>
        </div>

        <div className="bg-gray-50 rounded-xl p-4">
          <div className="flex items-center gap-2 text-gray-500 mb-2">
            <Zap size={14} />
            <span className="text-xs font-medium">Version</span>
          </div>
          <p className="text-sm font-medium text-gray-900">v{agent.manifest.version}</p>
        </div>
      </div>

      {/* Details */}
      <div className="bg-gray-50 rounded-xl p-4 space-y-3">
        <h3 className="text-xs font-medium text-gray-500 uppercase tracking-wide">Agent Info</h3>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-sm">
          <div className="flex items-center gap-2">
            <Globe size={14} className="text-gray-400 shrink-0" />
            <span className="text-gray-500">URL</span>
            <code className="ml-auto text-xs text-gray-700 bg-white px-2 py-0.5 rounded font-mono">{agent.url}</code>
          </div>

          <div className="flex items-center gap-2">
            <Activity size={14} className="text-gray-400 shrink-0" />
            <span className="text-gray-500">Health Endpoint</span>
            <code className="ml-auto text-xs text-gray-700 bg-white px-2 py-0.5 rounded font-mono">{agent.manifest.health_endpoint}</code>
          </div>

          <div className="flex items-center gap-2">
            <Cpu size={14} className="text-gray-400 shrink-0" />
            <span className="text-gray-500">Capabilities</span>
            <div className="ml-auto flex gap-1">
              {agent.manifest.capabilities.map((c) => (
                <span key={c} className="text-xs bg-white text-gray-600 px-2 py-0.5 rounded font-medium">{c}</span>
              ))}
            </div>
          </div>

          <div className="flex items-center gap-2">
            <Clock size={14} className="text-gray-400 shrink-0" />
            <span className="text-gray-500">Last Health Check</span>
            <span className="ml-auto text-xs text-gray-700">
              {agent.last_health_check
                ? new Date(agent.last_health_check).toLocaleString()
                : "Never"}
            </span>
          </div>
        </div>
      </div>

      {/* Live health indicator */}
      <div className="bg-gray-50 rounded-xl p-4">
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-xs font-medium text-gray-500 uppercase tracking-wide">Live Health</h3>
          <span className="text-[10px] text-gray-400">polling every 10s</span>
        </div>
        <div className="flex items-center gap-3">
          <span className={`relative flex h-3 w-3`}>
            {health?.status === "healthy" && (
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75" />
            )}
            <span className={`relative inline-flex rounded-full h-3 w-3 ${
              health?.status === "healthy" ? "bg-green-500" : isError ? "bg-red-500" : "bg-gray-300"
            }`} />
          </span>
          <span className="text-sm text-gray-700">
            {isLoading
              ? "Checking..."
              : isError
                ? "Unreachable"
                : health?.status === "healthy"
                  ? "Agent is responding normally"
                  : `Status: ${health?.status ?? "unknown"}`}
          </span>
        </div>
      </div>

      {/* Manifest JSON (collapsible) */}
      <details className="bg-gray-50 rounded-xl">
        <summary className="p-4 text-xs font-medium text-gray-500 uppercase tracking-wide cursor-pointer select-none hover:text-gray-700">
          Raw Manifest
        </summary>
        <pre className="px-4 pb-4 text-xs text-gray-600 font-mono overflow-x-auto whitespace-pre-wrap">
          {JSON.stringify(agent.manifest, null, 2)}
        </pre>
      </details>
    </div>
  );
}

export default function WorkerAgentsPage() {
  const { data: agents, isLoading } = useAgents();
  const { data: microservices } = useMicroservices();

  const enabledAgents = useMemo(() => {
    if (!agents) return [];
    if (!microservices) return agents;
    const disabledIds = new Set(
      microservices.filter((ms) => !ms.enabled).map((ms) => ms.id)
    );
    return agents.filter((a) => !disabledIds.has(a.id));
  }, [agents, microservices]);

  const msMap = useMemo(() => {
    const map = new Map<string, Microservice>();
    (microservices ?? []).forEach((ms) => map.set(ms.id, ms));
    return map;
  }, [microservices]);

  const [selected, setSelected] = useState<Agent | null>(null);

  return (
    <div>
      <PageHeader title="Worker Agents" />

      {/* Mobile: show list or detail */}
      <div className="md:hidden">
        {selected ? (
          <div className="bg-white rounded-2xl shadow-sm overflow-hidden">
            <button
              onClick={() => setSelected(null)}
              className="flex items-center gap-2 px-4 pt-4 text-sm text-gray-500"
            >
              <ArrowLeft size={16} />
              All Agents
            </button>
            <AgentDetail
              key={selected.id}
              agent={selected}
              microservice={msMap.get(selected.id)}
            />
          </div>
        ) : (
          <div className="bg-white rounded-2xl shadow-sm p-4">
            {isLoading && <p className="text-sm text-gray-400 p-2">Loading...</p>}
            {enabledAgents.map((agent) => (
              <button
                key={agent.id}
                onClick={() => setSelected(agent)}
                className="w-full flex items-center gap-3 p-3 rounded-xl text-left transition-colors hover:bg-gray-50"
              >
                <Bot size={18} className="text-gray-500 shrink-0" />
                <div className="min-w-0">
                  <p className="text-sm font-medium text-gray-900 truncate">{agent.name}</p>
                  <p className="text-xs text-gray-400">v{agent.manifest.version}</p>
                </div>
                <span
                  className={`ml-auto w-2 h-2 rounded-full shrink-0 ${
                    agent.status === "healthy" ? "bg-green-400" : "bg-red-400"
                  }`}
                />
              </button>
            ))}
            {enabledAgents.length === 0 && !isLoading && (
              <p className="text-sm text-gray-400 p-2">No agents registered</p>
            )}
          </div>
        )}
      </div>

      {/* Desktop: side-by-side */}
      <div className="hidden md:flex gap-6 h-[calc(100vh-12rem)]">
        {/* Agent list */}
        <div className="w-72 bg-white rounded-2xl shadow-sm p-4 overflow-y-auto shrink-0">
          {isLoading && <p className="text-sm text-gray-400 p-2">Loading...</p>}
          {enabledAgents.map((agent) => (
            <button
              key={agent.id}
              onClick={() => setSelected(agent)}
              className={`w-full flex items-center gap-3 p-3 rounded-xl text-left transition-colors ${
                selected?.id === agent.id ? "bg-gray-100" : "hover:bg-gray-50"
              }`}
            >
              <Bot size={18} className="text-gray-500 shrink-0" />
              <div className="min-w-0">
                <p className="text-sm font-medium text-gray-900 truncate">{agent.name}</p>
                <p className="text-xs text-gray-400">v{agent.manifest.version}</p>
              </div>
              <span
                className={`ml-auto w-2 h-2 rounded-full shrink-0 ${
                  agent.status === "healthy" ? "bg-green-400" : "bg-red-400"
                }`}
              />
            </button>
          ))}
          {enabledAgents.length === 0 && !isLoading && (
            <p className="text-sm text-gray-400 p-2">No agents registered</p>
          )}
        </div>

        {/* Agent detail */}
        <div className="flex-1 bg-white rounded-2xl shadow-sm overflow-hidden">
          {selected ? (
            <AgentDetail
              key={selected.id}
              agent={selected}
              microservice={msMap.get(selected.id)}
            />
          ) : (
            <div className="flex items-center justify-center h-full text-gray-400 text-sm">
              Select an agent to view
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
