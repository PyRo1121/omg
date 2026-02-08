import { Router } from "@solidjs/router";
import { FileRoutes } from "@solidjs/start/router";
import { Suspense, onMount } from "solid-js";
import { MetaProvider } from "@solidjs/meta";
import { QueryClientProvider } from "@tanstack/solid-query";
import { queryClient } from "./lib/query";
import { initAnalytics } from "./lib/analytics-client";
import "./app.css";

export default function App() {
  onMount(() => {
    initAnalytics();
  });

  return (
    <Router
      root={(props) => (
        <MetaProvider>
          <QueryClientProvider client={queryClient}>
            <Suspense fallback={<PageLoader />}>{props.children}</Suspense>
          </QueryClientProvider>
        </MetaProvider>
      )}
    >
      <FileRoutes />
    </Router>
  );
}

function PageLoader() {
  return (
    <div class="flex min-h-screen items-center justify-center bg-[#0a0a0a]">
      <div class="h-8 w-8 animate-spin rounded-full border-2 border-blue-500 border-t-transparent" />
    </div>
  );
}
