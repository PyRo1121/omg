import { Component, For } from 'solid-js';

const Features: Component = () => {
  return (
    <section id="features" class="py-32 px-6 relative" aria-labelledby="features-heading">
      {/* Background accent */}
      <div class="absolute inset-0 bg-gradient-to-b from-transparent via-indigo-500/5 to-transparent pointer-events-none" />
      
      <div class="max-w-7xl mx-auto relative">
        {/* Section header */}
        <div class="text-center mb-20">
          <div class="inline-flex items-center gap-2 px-4 py-2 rounded-full bg-indigo-500/10 border border-indigo-500/20 text-sm text-indigo-300 mb-6">
            <span>Why OMG?</span>
          </div>
          <h2 id="features-heading" class="text-4xl md:text-5xl lg:text-6xl font-bold mb-6">
            One Tool to <span class="gradient-text">Rule Them All</span>
          </h2>
          <p class="text-xl text-slate-400 max-w-3xl mx-auto leading-relaxed">
            Stop juggling between pacman, yay, nvm, pyenv, and rbenv. OMG unifies everything into a single, 
            blazing-fast CLI that's 50-200x faster than the alternatives.
          </p>
        </div>

        {/* Main feature grid */}
        <div class="grid lg:grid-cols-3 gap-8 mb-20">
          {/* Speed */}
          <div class="feature-card group relative overflow-hidden">
            <div class="absolute top-0 right-0 w-32 h-32 bg-gradient-to-br from-yellow-500/20 to-orange-500/20 rounded-full blur-3xl group-hover:scale-150 transition-transform duration-500" />
            <div class="relative">
              <div class="w-16 h-16 rounded-2xl bg-gradient-to-br from-yellow-400 to-orange-500 flex items-center justify-center mb-6 shadow-lg shadow-orange-500/25">
                <svg class="w-8 h-8 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
                </svg>
              </div>
              <h3 class="text-2xl font-bold mb-3">Blazing Fast</h3>
              <p class="text-slate-400 mb-4">
                6ms average query time. Direct libalpm access means we're 22x faster than pacman and 200x faster than yay.
              </p>
              <div class="flex items-center gap-4 text-sm">
                <div class="flex items-center gap-1">
                  <span class="text-cyan-400 font-mono font-bold">6ms</span>
                  <span class="text-slate-500">omg</span>
                </div>
                <div class="flex items-center gap-1">
                  <span class="text-slate-400 font-mono">132ms</span>
                  <span class="text-slate-500">pacman</span>
                </div>
                <div class="flex items-center gap-1">
                  <span class="text-slate-400 font-mono">1.3s</span>
                  <span class="text-slate-500">yay</span>
                </div>
              </div>
            </div>
          </div>

          {/* Runtimes */}
          <div class="feature-card group relative overflow-hidden">
            <div class="absolute top-0 right-0 w-32 h-32 bg-gradient-to-br from-cyan-500/20 to-blue-500/20 rounded-full blur-3xl group-hover:scale-150 transition-transform duration-500" />
            <div class="relative">
              <div class="w-16 h-16 rounded-2xl bg-gradient-to-br from-cyan-400 to-blue-500 flex items-center justify-center mb-6 shadow-lg shadow-blue-500/25">
                <svg class="w-8 h-8 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
                </svg>
              </div>
              <h3 class="text-2xl font-bold mb-3">Native Core + 100+ via mise</h3>
              <p class="text-slate-400 mb-4">
                Built-in runtime managers for Node, Python, Go, Rust, Ruby, Java, and Bun. The long-tail (Zig, Elixir, Dart, and more) flows through mise as we expand.
              </p>
              <div class="flex flex-wrap gap-2">
                <span class="px-2 py-1 bg-green-500/20 text-green-400 rounded text-xs font-mono">node</span>
                <span class="px-2 py-1 bg-blue-500/20 text-blue-400 rounded text-xs font-mono">python</span>
                <span class="px-2 py-1 bg-cyan-500/20 text-cyan-400 rounded text-xs font-mono">go</span>
                <span class="px-2 py-1 bg-orange-500/20 text-orange-400 rounded text-xs font-mono">rust</span>
                <span class="px-2 py-1 bg-red-500/20 text-red-400 rounded text-xs font-mono">ruby</span>
                <span class="px-2 py-1 bg-yellow-500/20 text-yellow-400 rounded text-xs font-mono">java</span>
                <span class="px-2 py-1 bg-pink-500/20 text-pink-400 rounded text-xs font-mono">bun</span>
                <span class="px-2 py-1 bg-purple-500/20 text-purple-400 rounded text-xs font-mono">zig</span>
                <span class="px-2 py-1 bg-indigo-500/20 text-indigo-400 rounded text-xs font-mono">elixir</span>
                <span class="px-2 py-1 bg-teal-500/20 text-teal-400 rounded text-xs font-mono">dart</span>
                <span class="px-2 py-1 bg-slate-700 text-slate-300 rounded text-xs font-mono">+100 via mise</span>
              </div>
            </div>
          </div>

          {/* Security */}
          <div class="feature-card group relative overflow-hidden">
            <div class="absolute top-0 right-0 w-32 h-32 bg-gradient-to-br from-red-500/20 to-pink-500/20 rounded-full blur-3xl group-hover:scale-150 transition-transform duration-500" />
            <div class="relative">
              <div class="w-16 h-16 rounded-2xl bg-gradient-to-br from-red-400 to-pink-500 flex items-center justify-center mb-6 shadow-lg shadow-pink-500/25">
                <svg class="w-8 h-8 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                </svg>
              </div>
              <h3 class="text-2xl font-bold mb-3">Enterprise Security</h3>
              <p class="text-slate-400 mb-4">
                SBOM generation (CycloneDX 1.5), vulnerability scanning, secret detection, and tamper-proof audit logs.
              </p>
              <div class="flex flex-wrap gap-2">
                <span class="px-2 py-1 bg-slate-700 text-slate-300 rounded text-xs">SBOM</span>
                <span class="px-2 py-1 bg-slate-700 text-slate-300 rounded text-xs">CVE Scanning</span>
                <span class="px-2 py-1 bg-slate-700 text-slate-300 rounded text-xs">SLSA</span>
              </div>
            </div>
          </div>
        </div>

        {/* Secondary features */}
        <div class="grid md:grid-cols-2 lg:grid-cols-4 gap-6">
          <div class="p-6 rounded-xl bg-slate-800/50 border border-slate-700/50 hover:border-indigo-500/50 transition-colors">
            <div class="w-12 h-12 rounded-xl bg-purple-500/20 flex items-center justify-center mb-4">
              <svg class="w-6 h-6 text-purple-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
              </svg>
            </div>
            <h4 class="font-semibold mb-2">Team Sync</h4>
            <p class="text-sm text-slate-400">Shared environment locks with drift detection for your entire team.</p>
          </div>

          <div class="p-6 rounded-xl bg-slate-800/50 border border-slate-700/50 hover:border-indigo-500/50 transition-colors">
            <div class="w-12 h-12 rounded-xl bg-blue-500/20 flex items-center justify-center mb-4">
              <svg class="w-6 h-6 text-blue-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 3v4M3 5h4M6 17v4m-2-2h4m5-16l2.286 6.857L21 12l-5.714 2.143L13 21l-2.286-6.857L5 12l5.714-2.143L13 3z" />
              </svg>
            </div>
            <h4 class="font-semibold mb-2">Container Integration</h4>
            <p class="text-sm text-slate-400">Docker/Podman support with auto-detection and dev shells.</p>
          </div>

          <div class="p-6 rounded-xl bg-slate-800/50 border border-slate-700/50 hover:border-indigo-500/50 transition-colors">
            <div class="w-12 h-12 rounded-xl bg-green-500/20 flex items-center justify-center mb-4">
              <svg class="w-6 h-6 text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            </div>
            <h4 class="font-semibold mb-2">Task Runner</h4>
            <p class="text-sm text-slate-400">Auto-detects package.json, Cargo.toml, Makefile, and 10+ project types.</p>
          </div>

          <div class="p-6 rounded-xl bg-slate-800/50 border border-slate-700/50 hover:border-indigo-500/50 transition-colors">
            <div class="w-12 h-12 rounded-xl bg-yellow-500/20 flex items-center justify-center mb-4">
              <svg class="w-6 h-6 text-yellow-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
              </svg>
            </div>
            <h4 class="font-semibold mb-2">Environment Capture</h4>
            <p class="text-sm text-slate-400">Fingerprint your entire dev environment and share via Gist.</p>
          </div>
        </div>
      </div>
    </section>
  );
};

export default Features;
