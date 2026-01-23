import { Component, JSX, Show, createEffect, onCleanup } from 'solid-js';
import { Portal } from 'solid-js/web';

interface DialogProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  children?: JSX.Element;
  confirmLabel?: string;
  cancelLabel?: string;
  onConfirm?: () => void;
  variant?: 'danger' | 'primary' | 'neutral';
  loading?: boolean;
}

export const Dialog: Component<DialogProps> = (props) => {
  let ref: HTMLDivElement | undefined;

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') props.onClose();
  };

  createEffect(() => {
    if (props.isOpen) {
      document.body.style.overflow = 'hidden';
      window.addEventListener('keydown', handleKeyDown);
    } else {
      document.body.style.overflow = '';
      window.removeEventListener('keydown', handleKeyDown);
    }
  });

  onCleanup(() => {
    document.body.style.overflow = '';
    window.removeEventListener('keydown', handleKeyDown);
  });

  return (
    <Show when={props.isOpen}>
      <Portal>
        <div class="fixed inset-0 z-[999] flex items-center justify-center p-4 sm:p-6" role="dialog" aria-modal="true" aria-labelledby="dialog-title">
          <div class="absolute inset-0 bg-black/60 backdrop-blur-sm transition-opacity" onClick={props.onClose} />
          
          <div ref={ref} class="relative w-full max-w-lg transform overflow-hidden rounded-[2rem] border border-white/10 bg-[#0a0a0b] p-8 text-left shadow-2xl transition-all animate-in zoom-in-95 duration-200">
            <div class="mb-6">
              <h3 id="dialog-title" class="text-xl font-black text-white tracking-tight">
                {props.title}
              </h3>
              <Show when={props.description}>
                <p class="mt-2 text-sm text-slate-400">
                  {props.description}
                </p>
              </Show>
            </div>

            <div class="mb-8">
              {props.children}
            </div>

            <div class="flex flex-col-reverse sm:flex-row sm:justify-end gap-3">
              <button
                type="button"
                class="inline-flex justify-center rounded-xl border border-white/10 bg-white/5 px-6 py-3 text-sm font-bold text-white shadow-sm hover:bg-white/10 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 focus:ring-offset-gray-900 transition-colors"
                onClick={props.onClose}
                disabled={props.loading}
              >
                {props.cancelLabel || 'Cancel'}
              </button>
              <Show when={props.onConfirm}>
                <button
                  type="button"
                  class={`inline-flex justify-center rounded-xl px-6 py-3 text-sm font-black text-white shadow-sm focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-offset-gray-900 transition-all ${
                    props.variant === 'danger' 
                      ? 'bg-rose-600 hover:bg-rose-500 focus:ring-rose-500' 
                      : 'bg-indigo-600 hover:bg-indigo-500 focus:ring-indigo-500'
                  } ${props.loading ? 'opacity-50 cursor-not-allowed' : ''}`}
                  onClick={props.onConfirm}
                  disabled={props.loading}
                >
                  {props.loading ? 'Processing...' : props.confirmLabel || 'Confirm'}
                </button>
              </Show>
            </div>
          </div>
        </div>
      </Portal>
    </Show>
  );
};
