import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router";
import PageHeader from "../components/PageHeader";
import { useAgents } from "../hooks/useAgents";
import { useMicroservices } from "../hooks/useMicroservices";

export default function MicroserviceAppPage() {
  const { slug } = useParams<{ slug: string }>();
  const { data: microservices } = useMicroservices();
  const { data: agents } = useAgents();
  const containerRef = useRef<HTMLDivElement>(null);
  const unmountRef = useRef<((el: HTMLElement) => void) | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const prevSlugRef = useRef<string | undefined>(undefined);

  const ms = microservices?.find((m) => m.slug === slug);
  const agent = agents?.find((a) => a.id === ms?.id);
  const manifest = agent?.manifest;

  useEffect(() => {
    const container = containerRef.current;
    if (!container || !manifest?.ui || !agent) return;

    // Reset state when slug changes
    if (prevSlugRef.current !== slug) {
      if (unmountRef.current && container) {
        unmountRef.current(container);
        unmountRef.current = null;
      }
      setError(null);
      setLoading(true);
      prevSlugRef.current = slug;
    }

    let cancelled = false;
    const proxyBase = `/api/agents/${agent.id}/proxy`;

    // Load CSS if specified
    let link: HTMLLinkElement | null = null;
    if (manifest.ui.bundle_css) {
      link = document.createElement("link");
      link.rel = "stylesheet";
      link.href = `${proxyBase}${manifest.ui.entry_path}${manifest.ui.bundle_css}`;
      document.head.appendChild(link);
    }

    // Load JS bundle
    const jsPath = manifest.ui.bundle_js;
    if (jsPath) {
      import(/* @vite-ignore */ `${proxyBase}${manifest.ui.entry_path}${jsPath}`)
        .then((mod) => {
          if (cancelled) return;
          mod.mount(container, { apiBase: proxyBase });
          unmountRef.current = mod.unmount;
          setLoading(false);
        })
        .catch((err) => {
          if (cancelled) return;
          console.error(`Failed to load ${ms?.name ?? slug} app:`, err);
          setError(`Failed to load ${ms?.name ?? slug}. Is the agent running?`);
          setLoading(false);
        });
    } else {
      setError("This service has no JS bundle configured in its manifest.");
      setLoading(false);
    }

    return () => {
      cancelled = true;
      if (unmountRef.current && container) {
        unmountRef.current(container);
        unmountRef.current = null;
      }
      if (link) link.remove();
    };
  }, [agent, manifest, slug, ms?.name]);

  if (!ms) {
    return (
      <div>
        <PageHeader title="Service Not Found" />
        <div className="flex items-center justify-center h-[calc(100vh-10rem)] md:h-[calc(100vh-12rem)] text-gray-400 text-sm">
          No service found with slug "{slug}"
        </div>
      </div>
    );
  }

  return (
    <div>
      <PageHeader title={ms.name} />

      {loading && !error && (
        <div className="flex items-center justify-center h-[calc(100vh-10rem)] md:h-[calc(100vh-12rem)] text-gray-400 text-sm">
          Loading {ms.name}...
        </div>
      )}

      {error && (
        <div className="flex items-center justify-center h-[calc(100vh-10rem)] md:h-[calc(100vh-12rem)] text-red-400 text-sm">
          {error}
        </div>
      )}

      <div
        id="ms-mount"
        ref={containerRef}
        className="h-[calc(100vh-10rem)] md:h-[calc(100vh-12rem)]"
        style={{ display: loading || error ? "none" : undefined }}
      />
    </div>
  );
}
