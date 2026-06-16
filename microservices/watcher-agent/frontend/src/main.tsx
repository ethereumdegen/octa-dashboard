import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./App";
import { setBasePath } from "./api/client";
import "./styles/globals.css";

const roots = new WeakMap<HTMLElement, ReactDOM.Root>();

export interface MountOptions {
  apiBase?: string;
}

export function mount(container: HTMLElement, options?: MountOptions) {
  if (options?.apiBase) {
    setBasePath(options.apiBase);
  }

  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: 1, refetchOnWindowFocus: false },
    },
  });

  const root = ReactDOM.createRoot(container);
  root.render(
    <React.StrictMode>
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>
    </React.StrictMode>,
  );

  roots.set(container, root);
  return root;
}

export function unmount(container: HTMLElement) {
  const root = roots.get(container);
  if (root) {
    root.unmount();
    roots.delete(container);
  }
}
