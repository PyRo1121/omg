import { Component } from 'solid-js';

const Features: Component = () => {
  return (
    <section id="features" class="relative px-6 py-32" aria-labelledby="features-heading">
      {/* Background accent */}
      <div class="pointer-events-none absolute inset-0 bg-gradient-to-b from-transparent via-indigo-500/5 to-transparent" />

      <div class="relative mx-auto max-w-7xl">
        {/* Section header */}
        <div class="mb-20 text-center">
          <div class="mb-6 inline-flex items-center gap-2 rounded-full border border-indigo-500/20 bg-indigo-500/10 px-4 py-2 text-sm text-indigo-300">
            <span>Why OMG?</span>
          </div>
          <h2 id="features-heading" class="mb-6 text-4xl font-bold md:text-5xl lg:text-6xl">
            One Tool to <span class="gradient-text">Rule Them All</span>
          </h2>
          <p class="mx-auto max-w-3xl text-xl leading-relaxed text-slate-400">
            Stop juggling between pacman, yay, nvm, pyenv, and rbenv. OMG unifies everything into a
            single, blazing-fast CLI that's 22x faster than the alternatives.
          </p>
        </div>

        {/* Main feature grid */}
        <div class="mb-20 grid gap-8 lg:grid-cols-3">
          {/* Speed */}
          <div class="feature-card group relative overflow-hidden">
            <div class="absolute top-0 right-0 h-32 w-32 rounded-full bg-gradient-to-br from-yellow-500/20 to-orange-500/20 blur-3xl transition-transform duration-500 group-hover:scale-150" />
            <div class="relative">
              <div class="mb-6 flex h-16 w-16 items-center justify-center rounded-2xl bg-gradient-to-br from-yellow-400 to-orange-500 shadow-lg shadow-orange-500/25">
                <svg
                  class="h-8 w-8 text-white"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M13 10V3L4 14h7v7l9-11h-7z"
                  />
                </svg>
              </div>
              <h3 class="mb-3 text-2xl font-bold">Blazing Fast</h3>
              <p class="mb-4 text-slate-400">
                6ms average query time. Direct libalpm access and in-memory indexing means we're 22x
                faster than pacman.
              </p>
              <div class="flex items-center gap-4 text-sm">
                <div class="flex items-center gap-1">
                  <span class="font-mono font-bold text-cyan-400">6ms</span>
                  <span class="text-slate-500">omg</span>
                </div>
                <div class="flex items-center gap-1">
                  <span class="font-mono text-slate-400">132ms</span>
                  <span class="text-slate-500">pacman</span>
                </div>
                <div class="flex items-center gap-1">
                  <span class="font-mono text-slate-400">150ms</span>
                  <span class="text-slate-500">yay</span>
                </div>
              </div>
            </div>
          </div>

          {/* Runtimes */}
          <div class="feature-card group relative overflow-hidden">
            <div class="absolute top-0 right-0 h-32 w-32 rounded-full bg-gradient-to-br from-cyan-500/20 to-blue-500/20 blur-3xl transition-transform duration-500 group-hover:scale-150" />
            <div class="relative">
              <div class="mb-6 flex h-16 w-16 items-center justify-center rounded-2xl bg-gradient-to-br from-cyan-400 to-blue-500 shadow-lg shadow-blue-500/25">
                <svg
                  class="h-8 w-8 text-white"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"
                  />
                </svg>
              </div>
              <h3 class="mb-3 text-2xl font-bold">Native Core + 100+ via mise</h3>
              <p class="mb-4 text-slate-400">
                Built-in runtime managers for Node, Python, Go, Rust, Ruby, Java, and Bun. The
                long-tail (Zig, Elixir, Dart, and more) flows through mise as we expand.
              </p>
              <div class="flex flex-wrap gap-2">
                <span class="rounded bg-green-500/20 px-2 py-1 font-mono text-xs text-green-400">
                  node
                </span>
                <span class="rounded bg-blue-500/20 px-2 py-1 font-mono text-xs text-blue-400">
                  python
                </span>
                <span class="rounded bg-cyan-500/20 px-2 py-1 font-mono text-xs text-cyan-400">
                  go
                </span>
                <span class="rounded bg-orange-500/20 px-2 py-1 font-mono text-xs text-orange-400">
                  rust
                </span>
                <span class="rounded bg-red-500/20 px-2 py-1 font-mono text-xs text-red-400">
                  ruby
                </span>
                <span class="rounded bg-yellow-500/20 px-2 py-1 font-mono text-xs text-yellow-400">
                  java
                </span>
                <span class="rounded bg-pink-500/20 px-2 py-1 font-mono text-xs text-pink-400">
                  bun
                </span>
                <span class="rounded bg-purple-500/20 px-2 py-1 font-mono text-xs text-purple-400">
                  zig
                </span>
                <span class="rounded bg-indigo-500/20 px-2 py-1 font-mono text-xs text-indigo-400">
                  elixir
                </span>
                <span class="rounded bg-teal-500/20 px-2 py-1 font-mono text-xs text-teal-400">
                  dart
                </span>
                <span class="rounded bg-slate-700 px-2 py-1 font-mono text-xs text-slate-300">
                  +100 via mise
                </span>
              </div>
            </div>
          </div>

          {/* Security */}
          <div class="feature-card group relative overflow-hidden">
            <div class="absolute top-0 right-0 h-32 w-32 rounded-full bg-gradient-to-br from-red-500/20 to-pink-500/20 blur-3xl transition-transform duration-500 group-hover:scale-150" />
            <div class="relative">
              <div class="mb-6 flex h-16 w-16 items-center justify-center rounded-2xl bg-gradient-to-br from-red-400 to-pink-500 shadow-lg shadow-pink-500/25">
                <svg
                  class="h-8 w-8 text-white"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z"
                  />
                </svg>
              </div>
              <h3 class="mb-3 text-2xl font-bold">Enterprise Security</h3>
              <p class="mb-4 text-slate-400">
                SBOM generation (CycloneDX 1.5), vulnerability scanning, secret detection, and
                tamper-proof audit logs.
              </p>
              <div class="flex flex-wrap gap-2">
                <span class="rounded bg-slate-700 px-2 py-1 text-xs text-slate-300">SBOM</span>
                <span class="rounded bg-slate-700 px-2 py-1 text-xs text-slate-300">
                  CVE Scanning
                </span>
                <span class="rounded bg-slate-700 px-2 py-1 text-xs text-slate-300">SLSA</span>
              </div>
            </div>
          </div>
        </div>

        {/* Secondary features */}
        <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-4">
          <div class="rounded-xl border border-slate-700/50 bg-slate-800/50 p-6 transition-colors hover:border-indigo-500/50">
            <div class="mb-4 flex h-12 w-12 items-center justify-center rounded-xl bg-purple-500/20">
              <svg
                class="h-6 w-6 text-purple-400"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z"
                />
              </svg>
            </div>
            <h4 class="mb-2 font-semibold">Team Sync</h4>
            <p class="text-sm text-slate-400">
              Shared environment locks with drift detection for your entire team.
            </p>
          </div>

          <div class="rounded-xl border border-slate-700/50 bg-slate-800/50 p-6 transition-colors hover:border-indigo-500/50">
            <div class="mb-4 flex h-12 w-12 items-center justify-center rounded-xl bg-blue-500/20">
              <svg
                class="h-6 w-6 text-blue-400"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M5 3v4M3 5h4M6 17v4m-2-2h4m5-16l2.286 6.857L21 12l-5.714 2.143L13 21l-2.286-6.857L5 12l5.714-2.143L13 3z"
                />
              </svg>
            </div>
            <h4 class="mb-2 font-semibold">Container Integration</h4>
            <p class="text-sm text-slate-400">
              Docker/Podman support with auto-detection and dev shells.
            </p>
          </div>

          <div class="rounded-xl border border-slate-700/50 bg-slate-800/50 p-6 transition-colors hover:border-indigo-500/50">
            <div class="mb-4 flex h-12 w-12 items-center justify-center rounded-xl bg-green-500/20">
              <svg
                class="h-6 w-6 text-green-400"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z"
                />
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
            </div>
            <h4 class="mb-2 font-semibold">Task Runner</h4>
            <p class="text-sm text-slate-400">
              Auto-detects package.json, Cargo.toml, Makefile, and 10+ project types.
            </p>
          </div>

          <div class="rounded-xl border border-slate-700/50 bg-slate-800/50 p-6 transition-colors hover:border-indigo-500/50">
            <div class="mb-4 flex h-12 w-12 items-center justify-center rounded-xl bg-yellow-500/20">
              <svg
                class="h-6 w-6 text-yellow-400"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
                />
              </svg>
            </div>
            <h4 class="mb-2 font-semibold">Environment Capture</h4>
            <p class="text-sm text-slate-400">
              Fingerprint your entire dev environment and share via Gist.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
};

export default Features;
