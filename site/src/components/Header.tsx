import { Component, createSignal, onMount, onCleanup, Show } from 'solid-js';
import { A, useNavigate } from '@solidjs/router';
import { useSession } from '~/lib/auth-client';

const Header: Component = () => {
  const [menuOpen, setMenuOpen] = createSignal(false);
  const [showShortcuts, setShowShortcuts] = createSignal(false);
  const [userMenuOpen, setUserMenuOpen] = createSignal(false);
  const navigate = useNavigate();
  const session = useSession();

  // Global keyboard shortcuts and click outside handler
  onMount(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Don't trigger if typing in an input
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

      if (e.key === '?' && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        setShowShortcuts(prev => !prev);
      } else if (e.key === 'Escape') {
        setShowShortcuts(false);
        setUserMenuOpen(false);
      } else if (e.key === 'd' && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        navigate('/dashboard');
      }
    };

    const handleClickOutside = (e: MouseEvent) => {
      if (userMenuOpen() && !(e.target as Element).closest('.user-menu-container')) {
        setUserMenuOpen(false);
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    document.addEventListener('click', handleClickOutside);

    onCleanup(() => {
      document.removeEventListener('keydown', handleKeyDown);
      document.removeEventListener('click', handleClickOutside);
    });
  });

  return (
    <>
      <header class="pointer-events-auto fixed top-0 right-0 left-0 z-50 border-b border-white/5 bg-[#0f0f23]/80 backdrop-blur-lg">
        <nav class="relative mx-auto flex max-w-7xl items-center justify-between px-6 py-4">
          <A href="/" class="flex items-center gap-3" aria-label="OMG Package Manager - Home">
            <div class="flex h-8 w-8 items-center justify-center overflow-hidden rounded-lg">
              <img
                src="/logo-globe.png"
                alt="OMG Package Manager Logo"
                class="h-8 w-8 object-cover"
                width="32"
                height="32"
              />
            </div>
            <span class="text-xl font-bold">OMG</span>
          </A>

          <div class="hidden items-center gap-8 md:flex">
            <A
              href="/docs"
              class="text-slate-400 transition-colors hover:text-white"
              activeClass="text-white"
            >
              Docs
            </A>
            <a href="/#features" class="text-slate-400 transition-colors hover:text-white">
              Features
            </a>
            <a href="/#benchmarks" class="text-slate-400 transition-colors hover:text-white">
              Benchmarks
            </a>
            <a href="/#pricing" class="text-slate-400 transition-colors hover:text-white">
              Pricing
            </a>
            <a
              href="https://github.com/PyRo1121/omg/"
              target="_blank"
              rel="noopener noreferrer"
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
            <Show 
              when={session()?.user}
              fallback={
                <A href="/login" class="btn-secondary px-4 py-2 text-sm">
                  Sign In
                </A>
              }
            >
              <div class="user-menu-container relative">
                <button
                  onClick={() => setUserMenuOpen(!userMenuOpen())}
                  class="flex items-center gap-2 rounded-lg px-3 py-2 text-sm text-slate-400 transition-colors hover:bg-slate-800/50 hover:text-white"
                >
                  <div class="flex h-8 w-8 items-center justify-center rounded-full bg-indigo-500/20 text-indigo-400">
                    {session()?.user?.email?.[0].toUpperCase()}
                  </div>
                  <span class="max-w-[150px] truncate">{session()?.user?.email}</span>
                  <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                  </svg>
                </button>
                
                <Show when={userMenuOpen()}>
                  <div class="absolute right-0 top-full mt-2 w-48 rounded-lg border border-slate-700 bg-slate-900 shadow-xl">
                    <A 
                      href="/dashboard" 
                      class="block px-4 py-2 text-sm text-slate-300 transition-colors hover:bg-slate-800"
                      onClick={() => setUserMenuOpen(false)}
                    >
                      Dashboard
                    </A>
                    <a
                      href="/api/auth/sign-out"
                      class="block px-4 py-2 text-sm text-red-400 transition-colors hover:bg-slate-800"
                    >
                      Sign Out
                    </a>
                  </div>
                </Show>
              </div>
            </Show>
            <a href="/#install" class="btn-secondary px-4 py-2 text-sm">
              Install
            </a>
            <a href="/#pricing" class="btn-primary px-4 py-2 text-sm">
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
              <A href="/docs" class="text-slate-400 hover:text-white">
                Docs
              </A>
              <a href="/#features" class="text-slate-400 hover:text-white" onClick={() => setMenuOpen(false)}>
                Features
              </a>
              <a href="/#benchmarks" class="text-slate-400 hover:text-white" onClick={() => setMenuOpen(false)}>
                Benchmarks
              </a>
              <a href="/#pricing" class="text-slate-400 hover:text-white" onClick={() => setMenuOpen(false)}>
                Pricing
              </a>
              <a href="https://github.com/PyRo1121/omg/" class="text-slate-400 hover:text-white">
                GitHub
              </a>
              <Show 
                when={session()?.user}
                fallback={
                  <A href="/login" class="text-slate-400 hover:text-white">
                    Sign In
                  </A>
                }
              >
                <A href="/dashboard" class="text-slate-400 hover:text-white">
                  Dashboard
                </A>
                <a href="/api/auth/sign-out" class="text-red-400 hover:text-red-300">
                  Sign Out
                </a>
              </Show>
              <a href="/#install" class="btn-secondary px-4 py-2 text-center text-sm" onClick={() => setMenuOpen(false)}>
                Install
              </a>
              <a href="/#pricing" class="btn-primary px-4 py-2 text-center text-sm" onClick={() => setMenuOpen(false)}>
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
            </div>
          </div>
        </div>
      </Show>
    </>
  );
};

export default Header;
