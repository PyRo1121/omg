import { Component, JSX } from 'solid-js';
import { ErrorBoundary } from 'solid-js';
import { AlertCircle, RefreshCw } from 'lucide-solid';

interface TabErrorBoundaryProps {
  tab: string;
  children: JSX.Element;
}

export const TabErrorBoundary: Component<TabErrorBoundaryProps> = (props) => {
  return (
    <ErrorBoundary
      fallback={(err, reset) => (
        <div class="flex min-h-[400px] items-center justify-center rounded-3xl border border-red-500/20 bg-red-500/5 p-12">
          <div class="max-w-md text-center">
            <div class="mb-6 flex justify-center">
              <div class="rounded-2xl bg-red-500/10 p-4">
                <AlertCircle class="h-16 w-16 text-red-400" />
              </div>
            </div>
            <h3 class="mb-3 text-2xl font-black text-white">
              Failed to load {props.tab} tab
            </h3>
            <p class="mb-2 text-sm text-red-400">{err.message}</p>
            <p class="mb-6 text-xs text-nebula-500">
              The error has been logged. Try refreshing or contact support if the issue persists.
            </p>
            <button
              onClick={reset}
              class="inline-flex items-center gap-2 rounded-xl bg-red-500/20 px-6 py-3 font-bold text-red-400 transition-all hover:bg-red-500/30"
            >
              <RefreshCw class="h-4 w-4" />
              Retry Loading Tab
            </button>
          </div>
        </div>
      )}
    >
      {props.children}
    </ErrorBoundary>
  );
};
