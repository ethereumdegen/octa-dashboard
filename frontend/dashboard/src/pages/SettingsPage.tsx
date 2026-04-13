import { useState } from "react";
import { Key, Plus, Trash2, Copy, Check, Eye, EyeOff, Blocks, Shield, Globe, AlertTriangle } from "lucide-react";
import PageHeader from "../components/PageHeader";
import { useApiKeys, useApiKeyMutations } from "../hooks/useApiKeys";
import { useAuth } from "../hooks/useAuth";
import { useMicroserviceMutations } from "../hooks/useMicroservices";
import { usePlatformSecrets, usePlatformSecretMutations } from "../hooks/usePlatformSecrets";
import { useAgentSourceMutations } from "../hooks/useAgentSources";
import { useServices } from "../hooks/useServices";

export default function SettingsPage() {
  const { user } = useAuth();
  const { data: apiKeys, isLoading } = useApiKeys();
  const { create, revoke } = useApiKeyMutations();
  const { toggle } = useMicroserviceMutations();
  const { data: platformSecrets, isLoading: psLoading } = usePlatformSecrets();
  const { create: createSecret, set: setSecret, remove: removeSecret } = usePlatformSecretMutations();
  const { data: services, isLoading: svcLoading } = useServices();
  const { add: addSource, remove: removeSource } = useAgentSourceMutations();
  const isAdmin = user?.role === "admin";
  const [newKeyName, setNewKeyName] = useState("");
  const [createdKey, setCreatedKey] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [showKey, setShowKey] = useState(false);
  const [secretInputs, setSecretInputs] = useState<Record<string, string>>({});
  const [savedKeys, setSavedKeys] = useState<Record<string, boolean>>({});
  const [newSourceUrl, setNewSourceUrl] = useState("");
  const [newSecretKey, setNewSecretKey] = useState("");
  const [newSecretValue, setNewSecretValue] = useState("");
  const [newSecretDesc, setNewSecretDesc] = useState("");

  // Collect all missing secrets across all services
  const allMissingSecrets = services
    ?.flatMap((svc) =>
      (svc.missing_secrets ?? []).map((key) => ({ key, serviceName: svc.name, serviceId: svc.id }))
    )
    .filter((v, i, a) => a.findIndex((x) => x.key === v.key) === i) ?? [];

  const handleCreate = () => {
    if (!newKeyName.trim()) return;
    create.mutate(
      { name: newKeyName.trim() },
      {
        onSuccess: (data) => {
          setCreatedKey(data.key);
          setNewKeyName("");
          setShowKey(true);
        },
      }
    );
  };

  const handleCopy = () => {
    if (createdKey) {
      navigator.clipboard.writeText(createdKey);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const handleRevoke = (id: string, name: string) => {
    if (!confirm(`Revoke API key "${name}"? This cannot be undone.`)) return;
    revoke.mutate(id);
  };

  const handleCreateSecret = () => {
    const key = newSecretKey.trim().toUpperCase();
    const value = newSecretValue.trim();
    if (!key || !value) return;
    createSecret.mutate(
      { key, value, description: newSecretDesc.trim() },
      {
        onSuccess: () => {
          setNewSecretKey("");
          setNewSecretValue("");
          setNewSecretDesc("");
        },
      }
    );
  };

  return (
    <div>
      <PageHeader title="Settings" />

      <div className="max-w-2xl space-y-8">
        {/* Profile section */}
        <section className="bg-white rounded-2xl shadow-sm p-6">
          <h3 className="text-sm font-medium text-gray-500 mb-4">Profile</h3>
          <div className="flex items-center gap-4">
            <div className="w-12 h-12 rounded-full bg-gray-200 flex items-center justify-center text-lg font-medium text-gray-600">
              {user?.email?.[0]?.toUpperCase() ?? "?"}
            </div>
            <div>
              <p className="text-sm font-medium text-gray-900">{user?.email}</p>
              <p className="text-xs text-gray-500 capitalize">{user?.role}</p>
            </div>
          </div>
        </section>

        {/* API Keys section */}
        <section className="bg-white rounded-2xl shadow-sm p-6">
          <div className="flex items-center gap-2 mb-1">
            <Key size={16} className="text-gray-500" />
            <h3 className="text-sm font-medium text-gray-500">API Keys</h3>
          </div>
          <p className="text-xs text-gray-400 mb-6">
            Use API keys to authenticate programmatic access to platform APIs.
            Include the key as <code className="bg-gray-100 px-1 rounded">Authorization: Bearer tk_...</code>
          </p>

          {/* Missing secrets warning banner */}
          {isAdmin && allMissingSecrets.length > 0 && (
            <div className="mb-6 p-4 bg-amber-50 border border-amber-200 rounded-xl">
              <div className="flex items-start gap-2">
                <AlertTriangle size={16} className="text-amber-600 mt-0.5 shrink-0" />
                <div>
                  <p className="text-xs font-medium text-amber-800 mb-1">
                    Missing platform secrets
                  </p>
                  <p className="text-xs text-amber-700 mb-2">
                    Installed services are requesting secrets that haven't been configured yet.
                    Add them in the Platform Secrets section below.
                  </p>
                  <div className="flex flex-wrap gap-1.5">
                    {allMissingSecrets.map(({ key, serviceName }) => (
                      <span
                        key={key}
                        className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-mono bg-amber-100 text-amber-800"
                      >
                        {key}
                        <span className="text-amber-500 font-sans">({serviceName})</span>
                      </span>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* Create new key */}
          <div className="flex flex-col sm:flex-row gap-2 mb-6">
            <input
              type="text"
              value={newKeyName}
              onChange={(e) => setNewKeyName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleCreate()}
              placeholder="Key name (e.g. CI/CD, Obsidian sync)"
              className="flex-1 px-3 py-2 bg-gray-50 rounded-xl text-sm outline-none focus:ring-2 focus:ring-gray-200"
            />
            <button
              onClick={handleCreate}
              disabled={!newKeyName.trim() || create.isPending}
              className="px-4 py-2 bg-gray-900 text-white rounded-xl text-sm font-medium hover:bg-gray-800 transition-colors disabled:opacity-50"
            >
              <Plus size={14} className="inline mr-1" />
              Create
            </button>
          </div>

          {/* Newly created key banner */}
          {createdKey && (
            <div className="mb-6 p-4 bg-green-50 border border-green-200 rounded-xl">
              <p className="text-xs text-green-800 font-medium mb-2">
                Key created! Copy it now — you won't be able to see it again.
              </p>
              <div className="flex items-center gap-2">
                <code className="flex-1 text-xs bg-white px-3 py-2 rounded-lg border border-green-200 font-mono break-all">
                  {showKey ? createdKey : createdKey.slice(0, 11) + "..." + "x".repeat(20)}
                </code>
                <button
                  onClick={() => setShowKey(!showKey)}
                  className="p-2 rounded-lg hover:bg-green-100 text-green-700 transition-colors"
                  title={showKey ? "Hide" : "Show"}
                >
                  {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
                </button>
                <button
                  onClick={handleCopy}
                  className="p-2 rounded-lg hover:bg-green-100 text-green-700 transition-colors"
                  title="Copy"
                >
                  {copied ? <Check size={14} /> : <Copy size={14} />}
                </button>
              </div>
            </div>
          )}

          {/* Key list */}
          {isLoading && <p className="text-sm text-gray-400">Loading...</p>}

          <div className="space-y-2">
            {apiKeys?.map((key) => (
              <div
                key={key.id}
                className="flex items-center justify-between p-3 bg-gray-50 rounded-xl group"
              >
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium text-gray-900">{key.name}</span>
                    <code className="text-xs text-gray-400 font-mono">{key.key_prefix}...{key.key_suffix}</code>
                  </div>
                  <div className="flex items-center gap-3 mt-1">
                    <span className="text-xs text-gray-400">
                      Created {new Date(key.created_at).toLocaleDateString()}
                    </span>
                    {key.last_used_at && (
                      <span className="text-xs text-gray-400">
                        Last used {new Date(key.last_used_at).toLocaleDateString()}
                      </span>
                    )}
                  </div>
                </div>
                <button
                  onClick={() => handleRevoke(key.id, key.name)}
                  className="sm:opacity-0 group-hover:opacity-100 p-2 rounded-lg text-gray-400 hover:text-red-500 hover:bg-red-50 transition-all"
                  title="Revoke"
                >
                  <Trash2 size={14} />
                </button>
              </div>
            ))}

            {!isLoading && apiKeys?.length === 0 && (
              <p className="text-sm text-gray-400 py-4 text-center">No API keys yet</p>
            )}
          </div>
        </section>

        {/* Unified Services section (admin only) */}
        {isAdmin && (
          <section className="bg-white rounded-2xl shadow-sm p-6">
            <div className="flex items-center gap-2 mb-1">
              <Blocks size={16} className="text-gray-500" />
              <h3 className="text-sm font-medium text-gray-500">Services</h3>
            </div>
            <p className="text-xs text-gray-400 mb-6">
              Add a service URL to auto-discover it. Toggle services on/off to control sidebar visibility.
            </p>

            {/* Add source URL */}
            <div className="flex flex-col sm:flex-row gap-2 mb-6">
              <input
                type="text"
                value={newSourceUrl}
                onChange={(e) => setNewSourceUrl(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && newSourceUrl.trim()) {
                    addSource.mutate(
                      { url: newSourceUrl.trim() },
                      { onSuccess: () => setNewSourceUrl("") }
                    );
                  }
                }}
                placeholder="http://localhost:4001"
                className="flex-1 px-3 py-2 bg-gray-50 rounded-xl text-sm outline-none focus:ring-2 focus:ring-gray-200 font-mono"
              />
              <button
                onClick={() => {
                  if (!newSourceUrl.trim()) return;
                  addSource.mutate(
                    { url: newSourceUrl.trim() },
                    { onSuccess: () => setNewSourceUrl("") }
                  );
                }}
                disabled={!newSourceUrl.trim() || addSource.isPending}
                className="px-4 py-2 bg-gray-900 text-white rounded-xl text-sm font-medium hover:bg-gray-800 transition-colors disabled:opacity-50"
              >
                <Plus size={14} className="inline mr-1" />
                Add Service
              </button>
            </div>

            {svcLoading && <p className="text-sm text-gray-400">Loading...</p>}

            <div className="space-y-3">
              {services?.map((svc) => (
                <div
                  key={svc.id}
                  className="p-4 bg-gray-50 rounded-xl"
                >
                  <div className="flex items-center justify-between mb-1.5">
                    <div className="flex items-center gap-2.5 min-w-0">
                      <span className="text-sm font-medium text-gray-900">{svc.name}</span>
                      <span className="text-xs text-gray-400 font-mono">/app/{svc.slug}</span>
                      {/* Health dot */}
                      <span
                        className={`w-2 h-2 rounded-full shrink-0 ${
                          svc.agent_status === "healthy"
                            ? "bg-green-400"
                            : svc.agent_status === "unhealthy"
                              ? "bg-red-400"
                              : "bg-gray-300"
                        }`}
                        title={svc.agent_status ?? "unknown"}
                      />
                    </div>
                    <button
                      onClick={() => toggle.mutate({ id: svc.id, enabled: !svc.enabled })}
                      disabled={toggle.isPending}
                      className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none disabled:opacity-50 ${
                        svc.enabled ? "bg-gray-900" : "bg-gray-200"
                      }`}
                    >
                      <span
                        className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out ${
                          svc.enabled ? "translate-x-5" : "translate-x-0"
                        }`}
                      />
                    </button>
                  </div>
                  <p className="text-xs text-gray-400">{svc.description}</p>
                  <div className="flex items-center gap-3 mt-2">
                    {svc.source_url && (
                      <code className="text-[11px] text-gray-400 font-mono">{svc.source_url}</code>
                    )}
                    {svc.last_health_check && (
                      <span className="text-[11px] text-gray-400">
                        checked {new Date(svc.last_health_check).toLocaleTimeString()}
                      </span>
                    )}
                  </div>
                  {/* Missing secrets warning */}
                  {svc.missing_secrets && svc.missing_secrets.length > 0 && (
                    <div className="mt-2 flex items-start gap-1.5">
                      <AlertTriangle size={12} className="text-amber-500 mt-0.5 shrink-0" />
                      <span className="text-[11px] text-amber-600">
                        Missing secrets:{" "}
                        {svc.missing_secrets.map((k, i) => (
                          <span key={k}>
                            {i > 0 && ", "}
                            <code className="bg-amber-50 px-1 rounded">{k}</code>
                          </span>
                        ))}
                      </span>
                    </div>
                  )}
                </div>
              ))}

              {!svcLoading && services?.length === 0 && (
                <div className="text-center py-8">
                  <Globe size={24} className="mx-auto text-gray-300 mb-2" />
                  <p className="text-sm text-gray-400">No services discovered yet</p>
                  <p className="text-xs text-gray-300 mt-1">Add a service URL above to get started</p>
                </div>
              )}
            </div>
          </section>
        )}

        {/* Platform Secrets section (admin only) */}
        {isAdmin && (
          <section className="bg-white rounded-2xl shadow-sm p-6">
            <div className="flex items-center gap-2 mb-1">
              <Shield size={16} className="text-gray-500" />
              <h3 className="text-sm font-medium text-gray-500">Platform Secrets</h3>
            </div>
            <p className="text-xs text-gray-400 mb-6">
              Store API keys and credentials that microservices can fetch at runtime.
              Values are never exposed in the UI after saving.
            </p>

            {/* Add new secret */}
            <div className="mb-6 p-4 bg-gray-50 rounded-xl space-y-2">
              <div className="flex flex-col sm:flex-row gap-2">
                <input
                  type="text"
                  value={newSecretKey}
                  onChange={(e) => setNewSecretKey(e.target.value.toUpperCase().replace(/[^A-Z0-9_]/g, ""))}
                  placeholder="KEY_NAME"
                  className="sm:w-40 px-3 py-2 bg-white rounded-lg text-sm outline-none focus:ring-2 focus:ring-gray-200 font-mono border border-gray-200"
                />
                <input
                  type="password"
                  value={newSecretValue}
                  onChange={(e) => setNewSecretValue(e.target.value)}
                  placeholder="Secret value"
                  className="flex-1 px-3 py-2 bg-white rounded-lg text-sm outline-none focus:ring-2 focus:ring-gray-200 font-mono border border-gray-200"
                />
                <button
                  onClick={handleCreateSecret}
                  disabled={!newSecretKey.trim() || !newSecretValue.trim() || createSecret.isPending}
                  className="px-4 py-2 bg-gray-900 text-white rounded-lg text-sm font-medium hover:bg-gray-800 transition-colors disabled:opacity-50"
                >
                  <Plus size={14} className="inline mr-1" />
                  Add
                </button>
              </div>
              <input
                type="text"
                value={newSecretDesc}
                onChange={(e) => setNewSecretDesc(e.target.value)}
                placeholder="Description (optional)"
                className="w-full px-3 py-1.5 bg-white rounded-lg text-xs outline-none focus:ring-2 focus:ring-gray-200 border border-gray-200 text-gray-500"
              />
            </div>

            {psLoading && <p className="text-sm text-gray-400">Loading...</p>}

            <div className="space-y-3">
              {platformSecrets?.map((secret) => (
                <div key={secret.key} className="p-4 bg-gray-50 rounded-xl">
                  <div className="flex items-center justify-between mb-1">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-gray-900 font-mono">{secret.key}</span>
                      <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium bg-green-100 text-green-700">
                        configured
                      </span>
                    </div>
                    <button
                      onClick={() => {
                        if (!confirm(`Remove ${secret.key}? Microservices using it will stop working.`)) return;
                        removeSecret.mutate(secret.key);
                      }}
                      className="p-1.5 rounded-lg text-gray-400 hover:text-red-500 hover:bg-red-50 transition-all"
                      title="Remove"
                    >
                      <Trash2 size={13} />
                    </button>
                  </div>
                  {secret.description && (
                    <p className="text-xs text-gray-400 mb-3">{secret.description}</p>
                  )}
                  <div className="flex gap-2">
                    <input
                      type="password"
                      value={secretInputs[secret.key] ?? ""}
                      onChange={(e) =>
                        setSecretInputs((prev) => ({ ...prev, [secret.key]: e.target.value }))
                      }
                      placeholder="Enter new value to update"
                      className="flex-1 px-3 py-1.5 bg-white rounded-lg text-sm outline-none focus:ring-2 focus:ring-gray-200 border border-gray-200 font-mono"
                    />
                    <button
                      onClick={() => {
                        const value = secretInputs[secret.key]?.trim();
                        if (!value) return;
                        setSecret.mutate(
                          { key: secret.key, value },
                          {
                            onSuccess: () => {
                              setSecretInputs((prev) => ({ ...prev, [secret.key]: "" }));
                              setSavedKeys((prev) => ({ ...prev, [secret.key]: true }));
                              setTimeout(() => setSavedKeys((prev) => ({ ...prev, [secret.key]: false })), 2000);
                            },
                          }
                        );
                      }}
                      disabled={!secretInputs[secret.key]?.trim() || setSecret.isPending}
                      className="px-3 py-1.5 bg-gray-900 text-white rounded-lg text-sm font-medium hover:bg-gray-800 transition-colors disabled:opacity-50"
                    >
                      {savedKeys[secret.key] ? <Check size={14} /> : "Save"}
                    </button>
                  </div>
                  {secret.updated_at && (
                    <p className="text-[11px] text-gray-400 mt-2">
                      Last updated {new Date(secret.updated_at).toLocaleDateString()}
                    </p>
                  )}
                </div>
              ))}

              {!psLoading && platformSecrets?.length === 0 && (
                <p className="text-sm text-gray-400 py-4 text-center">
                  No secrets configured yet. Add one above, or install a service that requires secrets.
                </p>
              )}
            </div>
          </section>
        )}
      </div>
    </div>
  );
}
