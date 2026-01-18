import { Component, createSignal, onMount, onCleanup, Show } from 'solid-js';
import Dashboard from './Dashboard';

const Header: Component = () => {
  const [menuOpen, setMenuOpen] = createSignal(false);
  const [showDashboard, setShowDashboard] = createSignal(false);
  const [showShortcuts, setShowShortcuts] = createSignal(false);

  const openDashboard = () => {
    setShowDashboard(true);
  };

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
        setShowDashboard(false);
      } else if (e.key === 'd' && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        openDashboard();
      }
    };
    
    document.addEventListener('keydown', handleKeyDown);
    
    onCleanup(() => document.removeEventListener('keydown', handleKeyDown));
  });

  return (
    <>
    <header class="fixed top-0 left-0 right-0 z-50 bg-[#0f0f23]/80 backdrop-blur-lg border-b border-white/5 pointer-events-auto">
      <nav class="max-w-7xl mx-auto px-6 py-4 flex items-center justify-between relative">
        <a href="/" class="flex items-center gap-3" aria-label="OMG Package Manager - Home">
          <div class="w-8 h-8 flex items-center justify-center">
            <img src="/favicon.svg" alt="OMG Package Manager Logo - Fastest Linux Package Manager" class="w-8 h-8 rounded-lg" width="32" height="32" />
          </div>
          <span class="text-xl font-bold">OMG</span>
        </a>

        <div class="hidden md:flex items-center gap-8">
          <a href="#features" class="text-slate-400 hover:text-white transition-colors">Features</a>
          <a href="#benchmarks" class="text-slate-400 hover:text-white transition-colors">Benchmarks</a>
          <a href="#pricing" class="text-slate-400 hover:text-white transition-colors">Pricing</a>
          <a href="https://github.com/PyRo1121/omg/" class="text-slate-400 hover:text-white transition-colors">GitHub</a>
        </div>

        <div class="hidden md:flex items-center gap-4">
          <button 
            onClick={() => setShowShortcuts(true)}
            class="text-slate-500 hover:text-white transition-colors text-xs px-2 py-1 border border-slate-700 rounded"
            title="Keyboard shortcuts (?)"
          >
            ?
          </button>
          <button 
            type="button"
            onClick={() => {
              console.log('Dashboard clicked');
              setShowDashboard(true);
            }}
            class="cursor-pointer text-slate-400 hover:text-white transition-colors text-sm flex items-center gap-1.5 px-3 py-2 rounded-lg hover:bg-slate-800/50 select-none"
            style={{ "user-select": "none" }}
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5.121 17.804A13.937 13.937 0 0112 16c2.5 0 4.847.655 6.879 1.804M15 10a3 3 0 11-6 0 3 3 0 016 0zm6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            Dashboard
          </button>
          <a href="#install" class="btn-secondary text-sm py-2 px-4">
            Install
          </a>
          <a href="#pricing" class="btn-primary text-sm py-2 px-4">
            Get Pro
          </a>
        </div>

        <button 
          class="md:hidden text-slate-400 hover:text-white"
          onClick={() => setMenuOpen(!menuOpen())}
        >
          <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
          </svg>
        </button>
      </nav>

      {menuOpen() && (
        <div class="md:hidden bg-[#1a1a2e] border-t border-white/5 px-6 py-4">
          <div class="flex flex-col gap-4">
            <a href="#features" class="text-slate-400 hover:text-white">Features</a>
            <a href="#benchmarks" class="text-slate-400 hover:text-white">Benchmarks</a>
            <a href="#pricing" class="text-slate-400 hover:text-white">Pricing</a>
            <a href="https://github.com/PyRo1121/omg/" class="text-slate-400 hover:text-white">GitHub</a>
            <button 
              onClick={() => setShowDashboard(true)}
              class="text-slate-400 hover:text-white text-left"
            >
              Dashboard
            </button>
            <a href="#install" class="btn-secondary text-sm py-2 px-4 text-center">Install</a>
            <a href="#pricing" class="btn-primary text-sm py-2 px-4 text-center">Get Pro</a>
          </div>
        </div>
      )}
    </header>
    <Dashboard isOpen={showDashboard()} onClose={() => setShowDashboard(false)} />
    
    {/* Keyboard Shortcuts Modal */}
    <Show when={showShortcuts()}>
      <div 
        class="fixed inset-0 z-[60] bg-black/60 backdrop-blur-sm flex items-center justify-center p-4"
        onClick={() => setShowShortcuts(false)}
      >
        <div 
          class="bg-slate-900 border border-slate-700 rounded-2xl p-6 max-w-md w-full shadow-2xl"
          onClick={(e) => e.stopPropagation()}
        >
          <div class="flex items-center justify-between mb-6">
            <h2 class="text-xl font-bold text-white flex items-center gap-2">
              ⌨️ Keyboard Shortcuts
            </h2>
            <button 
              onClick={() => setShowShortcuts(false)}
              class="text-slate-400 hover:text-white p-1"
            >
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
          
          <div class="space-y-4">
            <div class="text-sm text-slate-400 mb-4">Website Navigation</div>
            <div class="space-y-3">
              <div class="flex items-center justify-between">
                <span class="text-slate-300">Open Dashboard</span>
                <kbd class="px-2 py-1 bg-slate-800 border border-slate-600 rounded text-xs text-slate-300">D</kbd>
              </div>
              <div class="flex items-center justify-between">
                <span class="text-slate-300">Show Shortcuts</span>
                <kbd class="px-2 py-1 bg-slate-800 border border-slate-600 rounded text-xs text-slate-300">?</kbd>
              </div>
              <div class="flex items-center justify-between">
                <span class="text-slate-300">Close Modal</span>
                <kbd class="px-2 py-1 bg-slate-800 border border-slate-600 rounded text-xs text-slate-300">Esc</kbd>
              </div>
            </div>
            
            <div class="border-t border-slate-700 pt-4 mt-4">
              <div class="text-sm text-slate-400 mb-4">CLI Commands</div>
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
            
            <div class="border-t border-slate-700 pt-4 mt-4">
              <div class="text-sm text-slate-400 mb-4">TUI Dashboard (omg dash)</div>
              <div class="space-y-2 text-sm">
                <div class="flex items-center justify-between">
                  <span class="text-slate-300">Navigate tabs</span>
                  <div class="flex gap-1">
                    <kbd class="px-2 py-0.5 bg-slate-800 border border-slate-600 rounded text-xs">1</kbd>
                    <kbd class="px-2 py-0.5 bg-slate-800 border border-slate-600 rounded text-xs">2</kbd>
                    <kbd class="px-2 py-0.5 bg-slate-800 border border-slate-600 rounded text-xs">3</kbd>
                    <kbd class="px-2 py-0.5 bg-slate-800 border border-slate-600 rounded text-xs">4</kbd>
                    <kbd class="px-2 py-0.5 bg-slate-800 border border-slate-600 rounded text-xs">5</kbd>
                  </div>
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-slate-300">Search packages</span>
                  <kbd class="px-2 py-0.5 bg-slate-800 border border-slate-600 rounded text-xs">/</kbd>
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-slate-300">Update system</span>
                  <kbd class="px-2 py-0.5 bg-slate-800 border border-slate-600 rounded text-xs">U</kbd>
                </div>
                <div class="flex items-center justify-between">
                  <span class="text-slate-300">Quit</span>
                  <kbd class="px-2 py-0.5 bg-slate-800 border border-slate-600 rounded text-xs">Q</kbd>
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
