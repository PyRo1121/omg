import { Component, createSignal, onMount, onCleanup, Show } from 'solid-js';
import { A, useNavigate } from '@solidjs/router';

const Header: Component = () => {
  const [menuOpen, setMenuOpen] = createSignal(false);
  const [showShortcuts, setShowShortcuts] = createSignal(false);
  const navigate = useNavigate();

  // Global keyboard shortcuts
  onMount(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Don't trigger if typing in an input
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

      if (e.key === '?' && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        setShowShortcuts(prev => !prev);
      } else if (e.key === 'Escape') {
        setShowShortcuts(false);
      } else if (e.key === 'd' && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        navigate('/dashboard');
      }
    };

    document.addEventListener('keydown', handleKeyDown);

    onCleanup(() => document.removeEventListener('keydown', handleKeyDown));
  });

  return (
    <>
      <header class="pointer-events-auto fixed top-0 right-0 left-0 z-50 border-b border-white/5 bg-[#0f0f23]/80 backdrop-blur-lg">
        <nav class="relative mx-auto flex max-w-7xl items-center justify-between px-6 py-4">
          <a href="/" class="flex items-center gap-3" aria-label="OMG Package Manager - Home">
            <div class="flex h-8 w-8 items-center justify-center">
              <img
                src="/favicon.svg"
                alt="OMG Package Manager Logo - Fastest Linux Package Manager"
                class="h-8 w-8 rounded-lg"
                width="32"
                height="32"
              />
            </div>
            <span class="text-xl font-bold">OMG</span>
          </a>

          <div class="hidden items-center gap-8 md:flex">
            <a href="#features" class="text-slate-400 transition-colors hover:text-white">
              Features
            </a>
            <a href="#benchmarks" class="text-slate-400 transition-colors hover:text-white">
              Benchmarks
            </a>
            <a href="#pricing" class="text-slate-400 transition-colors hover:text-white">
              Pricing
            </a>
            <a
              href="https://github.com/PyRo1121/omg/"
              class="text-slate-400 transition-colors hover:text-white"
            >
              GitHub
            </a>
          </div>

          <div class="hidden items-center gap-4 md:flex">
            <button
              onClick={() => setShowShortcuts(true)}
              class="rounded border border-slate-700 px-2 py-1 text-xs text-slate-500 transition-colors hover:text-white"
              title="Keyboard shortcuts (?)"
            >
              ?
            </button>
            <A
              href="/dashboard"
              class="flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm text-slate-400 transition-colors hover:bg-slate-800/50 hover:text-white"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M5.121 17.804A13.937 13.937 0 0112 16c2.5 0 4.847.655 6.879 1.804M15 10a3 3 0 11-6 0 3 3 0 016 0zm6 2a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
              Dashboard
            </A>
            <a href="#install" class="btn-secondary px-4 py-2 text-sm">
              Install
            </a>
            <a href="#pricing" class="btn-primary px-4 py-2 text-sm">
              Get Pro
            </a>
          </div>

          <button
            class="text-slate-400 hover:text-white md:hidden"
            onClick={() => setMenuOpen(!menuOpen())}
          >
            <svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M4 6h16M4 12h16M4 18h16"
              />
            </svg>
          </button>
        </nav>

        {menuOpen() && (
          <div class="border-t border-white/5 bg-[#1a1a2e] px-6 py-4 md:hidden">
            <div class="flex flex-col gap-4">
              <a href="#features" class="text-slate-400 hover:text-white">
                Features
              </a>
              <a href="#benchmarks" class="text-slate-400 hover:text-white">
                Benchmarks
              </a>
              <a href="#pricing" class="text-slate-400 hover:text-white">
                Pricing
              </a>
              <a href="https://github.com/PyRo1121/omg/" class="text-slate-400 hover:text-white">
                GitHub
              </a>
              <A href="/dashboard" class="text-slate-400 hover:text-white">
                Dashboard
              </A>
              <a href="#install" class="btn-secondary px-4 py-2 text-center text-sm">
                Install
              </a>
              <a href="#pricing" class="btn-primary px-4 py-2 text-center text-sm">
                Get Pro
              </a>
            </div>
          </div>
        )}
      </header>

      {/* Keyboard Shortcuts Modal */}
      <Show when={showShortcuts()}>
        <div
          class="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm"
          onClick={() => setShowShortcuts(false)}
        >
          <div
            class="w-full max-w-md rounded-2xl border border-slate-700 bg-slate-900 p-6 shadow-2xl"
            onClick={e => e.stopPropagation()}
          >
            <div class="mb-6 flex items-center justify-between">
              <h2 class="flex items-center gap-2 text-xl font-bold text-white">
                ⌨️ Keyboard Shortcuts
              </h2>
              <button
                onClick={() => setShowShortcuts(false)}
                class="p-1 text-slate-400 hover:text-white"
              >
                <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </button>
            </div>

            <div class="space-y-4">
              <div class="mb-4 text-sm text-slate-400">Website Navigation</div>
              <div class="space-y-3">
                <div class="flex items-center justify-between">
                  <span class="text-slate-300">Open Dashboard</span>
                  <kbd class="rounded border border-slate-600 bg-slate-800 px-2 py-1 text-xs text-slate-300">
                    D
                  </kbd>
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-slate-300">Show Shortcuts</span>
                  <kbd class="rounded border border-slate-600 bg-slate-800 px-2 py-1 text-xs text-slate-300">
                    ?
                  </kbd>
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-slate-300">Close Modal</span>
                  <kbd class="rounded border border-slate-600 bg-slate-800 px-2 py-1 text-xs text-slate-300">
                    Esc
                  </kbd>
                </div>
              </div>

              <div class="mt-4 border-t border-slate-700 pt-4">
                <div class="mb-4 text-sm text-slate-400">CLI Commands</div>
                <div class="space-y-3 font-mono text-sm">
                  <div class="flex items-center justify-between">
                    <span class="text-cyan-400">omg s &lt;query&gt;</span>
                    <span class="text-slate-500">Search packages</span>
                  </div>
                  <div class="flex items-center justify-between">
                    <span class="text-cyan-400">omg i &lt;pkg&gt;</span>
                    <span class="text-slate-500">Install package</span>
                  </div>
                  <div class="flex items-center justify-between">
                    <span class="text-cyan-400">omg u</span>
                    <span class="text-slate-500">Update system</span>
                  </div>
                  <div class="flex items-center justify-between">
                    <span class="text-cyan-400">omg dash</span>
                    <span class="text-slate-500">Open TUI</span>
                  </div>
                  <div class="flex items-center justify-between">
                    <span class="text-cyan-400">omg stats</span>
                    <span class="text-slate-500">View usage stats</span>
                  </div>
                </div>
              </div>

              <div class="mt-4 border-t border-slate-700 pt-4">
                <div class="mb-4 text-sm text-slate-400">TUI Dashboard (omg dash)</div>
                <div class="space-y-2 text-sm">
                  <div class="flex items-center justify-between">
                    <span class="text-slate-300">Navigate tabs</span>
                    <div class="flex gap-1">
                      <kbd class="rounded border border-slate-600 bg-slate-800 px-2 py-0.5 text-xs">
                        1
                      </kbd>
                      <kbd class="rounded border border-slate-600 bg-slate-800 px-2 py-0.5 text-xs">
                        2
                      </kbd>
                      <kbd class="rounded border border-slate-600 bg-slate-800 px-2 py-0.5 text-xs">
                        3
                      </kbd>
                      <kbd class="rounded border border-slate-600 bg-slate-800 px-2 py-0.5 text-xs">
                        4
                      </kbd>
                      <kbd class="rounded border border-slate-600 bg-slate-800 px-2 py-0.5 text-xs">
                        5
                      </kbd>
                    </div>
                  </div>
                  <div class="flex items-center justify-between">
                    <span class="text-slate-300">Search packages</span>
                    <kbd class="rounded border border-slate-600 bg-slate-800 px-2 py-0.5 text-xs">
                      /
                    </kbd>
                  </div>
                  <div class="flex items-center justify-between">
                    <span class="text-slate-300">Update system</span>
                    <kbd class="rounded border border-slate-600 bg-slate-800 px-2 py-0.5 text-xs">
                      U
                    </kbd>
                  </div>
                  <div class="flex items-center justify-between">
                    <span class="text-slate-300">Quit</span>
                    <kbd class="rounded border border-slate-600 bg-slate-800 px-2 py-0.5 text-xs">
                      Q
                    </kbd>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </Show>
    </>
  );
};

export default Header;
