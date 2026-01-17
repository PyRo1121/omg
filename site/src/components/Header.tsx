import { Component, createSignal } from 'solid-js';

const Header: Component = () => {
  const [menuOpen, setMenuOpen] = createSignal(false);

  return (
    <header class="fixed top-0 left-0 right-0 z-50 bg-[#0f0f23]/80 backdrop-blur-lg border-b border-white/5">
      <nav class="max-w-7xl mx-auto px-6 py-4 flex items-center justify-between">
        <a href="/" class="flex items-center gap-3">
          <div class="w-8 h-8 flex items-center justify-center">
            <img src="/favicon.svg" alt="OMG" class="w-8 h-8 rounded-lg" />
          </div>
          <span class="text-xl font-bold">OMG</span>
        </a>

        <div class="hidden md:flex items-center gap-8">
          <a href="#features" class="text-slate-400 hover:text-white transition-colors">Features</a>
          <a href="#benchmarks" class="text-slate-400 hover:text-white transition-colors">Benchmarks</a>
          <a href="#pricing" class="text-slate-400 hover:text-white transition-colors">Pricing</a>
          <a href="https://github.com/omg-dev/omg" class="text-slate-400 hover:text-white transition-colors">GitHub</a>
        </div>

        <div class="hidden md:flex items-center gap-4">
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
            <a href="https://github.com/omg-dev/omg" class="text-slate-400 hover:text-white">GitHub</a>
            <a href="#install" class="btn-secondary text-sm py-2 px-4 text-center">Install</a>
            <a href="#pricing" class="btn-primary text-sm py-2 px-4 text-center">Get Pro</a>
          </div>
        </div>
      )}
    </header>
  );
};

export default Header;
