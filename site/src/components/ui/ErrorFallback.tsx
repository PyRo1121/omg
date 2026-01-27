import { Component } from 'solid-js';
import { AlertTriangle, RotateCcw } from 'lucide-solid';

export interface ErrorFallbackProps {
  error: Error;
  reset: () => void;
}

export const ErrorFallback: Component<ErrorFallbackProps> = (props) => {
  return (
    <div class="rounded-2xl border border-flare-500/20 bg-flare-500/10 p-8 text-center">
      <AlertTriangle class="mx-auto h-12 w-12 text-flare-400" />
      <h3 class="mt-4 text-lg font-bold text-white">Something went wrong</h3>
      <p class="mt-2 text-sm text-nebula-400">{props.error.message}</p>
      <button
        onClick={props.reset}
        class="mt-4 inline-flex items-center gap-2 rounded-xl bg-indigo-500 px-6 py-2 text-sm font-bold text-white transition-all hover:bg-indigo-600"
      >
        <RotateCcw size={14} />
        Try Again
      </button>
    </div>
  );
};

export default ErrorFallback;
