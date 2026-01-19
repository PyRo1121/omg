import { Component, For } from 'solid-js';

const RUNTIME_TAGS = [
  'node',
  'python',
  'go',
  'rust',
  'ruby',
  'java',
  'bun',
  'zig',
  'elixir',
  'dart',
  'deno',
  'php',
  'dotnet',
  'lua',
  'haskell',
  'erlang',
  'scala',
  'kotlin',
  'swift',
  'clojure',
];

const RuntimeEcosystem: Component = () => {
  return (
    <section id="runtimes" class="px-6 py-24">
      <div class="mx-auto max-w-7xl">
        <div class="mb-12 text-center">
          <div class="mb-4 inline-flex items-center gap-2 rounded-full border border-cyan-500/30 bg-cyan-500/10 px-4 py-2 text-sm font-semibold text-cyan-300">
            <span class="h-2 w-2 rounded-full bg-cyan-400" />
            Runtime Ecosystem
          </div>
          <h2 class="mb-4 text-4xl font-bold md:text-5xl">100+ Runtimes, One Workflow</h2>
          <p class="mx-auto max-w-3xl text-xl text-slate-400">
            OMG integrates with mise to unlock a massive runtime catalog, while keeping switching
            instant and project-aware.
          </p>
        </div>

        <div class="grid gap-6 lg:grid-cols-3">
          <div class="rounded-2xl border border-slate-700/60 bg-slate-800/60 p-6">
            <div class="mb-2 text-sm text-slate-400">Powered by mise</div>
            <h3 class="mb-3 text-2xl font-semibold">100+ runtime plugins</h3>
            <p class="text-slate-400">
              Install and switch versions for everything from Node, Python, and Go to Zig, Erlang,
              Swift, and more.
            </p>
          </div>
          <div class="rounded-2xl border border-slate-700/60 bg-slate-800/60 p-6">
            <div class="mb-2 text-sm text-slate-400">Zero lag</div>
            <h3 class="mb-3 text-2xl font-semibold">Shimless switching</h3>
            <p class="text-slate-400">
              Version switches happen in ~1.8ms with no shell pollution, no path hacks, and no
              startup drag.
            </p>
          </div>
          <div class="rounded-2xl border border-slate-700/60 bg-slate-800/60 p-6">
            <div class="mb-2 text-sm text-slate-400">Project-aware</div>
            <h3 class="mb-3 text-2xl font-semibold">Per-repo environments</h3>
            <p class="text-slate-400">
              Runtime versions are locked per project. Jump between repos and OMG swaps versions
              instantly.
            </p>
          </div>
        </div>

        <div class="mt-12 grid gap-8 lg:grid-cols-[1.3fr_1fr]">
          <div class="rounded-2xl border border-slate-700/60 bg-gradient-to-br from-slate-900/80 to-slate-800/60 p-6">
            <div class="mb-4 flex items-center justify-between">
              <h3 class="text-xl font-semibold">Popular runtimes</h3>
              <span class="text-sm text-slate-500">+100 more via mise</span>
            </div>
            <div class="flex flex-wrap gap-2">
              <For each={RUNTIME_TAGS}>
                {tag => (
                  <span class="rounded-full border border-slate-700/60 bg-slate-800 px-3 py-1 font-mono text-xs text-slate-300">
                    {tag}
                  </span>
                )}
              </For>
            </div>
          </div>

          <div class="rounded-2xl border border-slate-700/60 bg-slate-900 p-6">
            <div class="mb-3 text-sm text-slate-400">Runtime switches in practice</div>
            <div class="space-y-3 font-mono text-sm">
              <div class="flex items-center justify-between">
                <span class="text-slate-400">$ omg runtime use node@20</span>
                <span class="text-emerald-400">1.8ms</span>
              </div>
              <div class="text-slate-500">✔ node set to v20.11.1</div>
              <div class="flex items-center justify-between">
                <span class="text-slate-400">$ omg runtime use python@3.12</span>
                <span class="text-emerald-400">2.1ms</span>
              </div>
              <div class="text-slate-500">✔ python set to 3.12.1</div>
              <div class="flex items-center justify-between">
                <span class="text-slate-400">$ omg runtime use zig@0.12</span>
                <span class="text-emerald-400">2.0ms</span>
              </div>
              <div class="text-slate-500">✔ zig set to 0.12.0</div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};

export default RuntimeEcosystem;
