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
    <section id="runtimes" class="py-24 px-6">
      <div class="max-w-7xl mx-auto">
        <div class="text-center mb-12">
          <div class="inline-flex items-center gap-2 px-4 py-2 bg-cyan-500/10 border border-cyan-500/30 rounded-full text-cyan-300 text-sm font-semibold mb-4">
            <span class="w-2 h-2 rounded-full bg-cyan-400" />
            Runtime Ecosystem
          </div>
          <h2 class="text-4xl md:text-5xl font-bold mb-4">100+ Runtimes, One Workflow</h2>
          <p class="text-xl text-slate-400 max-w-3xl mx-auto">
            OMG integrates with mise to unlock a massive runtime catalog, while keeping switching instant and project-aware.
          </p>
        </div>

        <div class="grid lg:grid-cols-3 gap-6">
          <div class="p-6 rounded-2xl bg-slate-800/60 border border-slate-700/60">
            <div class="text-sm text-slate-400 mb-2">Powered by mise</div>
            <h3 class="text-2xl font-semibold mb-3">100+ runtime plugins</h3>
            <p class="text-slate-400">
              Install and switch versions for everything from Node, Python, and Go to Zig, Erlang, Swift, and more.
            </p>
          </div>
          <div class="p-6 rounded-2xl bg-slate-800/60 border border-slate-700/60">
            <div class="text-sm text-slate-400 mb-2">Zero lag</div>
            <h3 class="text-2xl font-semibold mb-3">Shimless switching</h3>
            <p class="text-slate-400">
              Version switches happen in ~1.8ms with no shell pollution, no path hacks, and no startup drag.
            </p>
          </div>
          <div class="p-6 rounded-2xl bg-slate-800/60 border border-slate-700/60">
            <div class="text-sm text-slate-400 mb-2">Project-aware</div>
            <h3 class="text-2xl font-semibold mb-3">Per-repo environments</h3>
            <p class="text-slate-400">
              Runtime versions are locked per project. Jump between repos and OMG swaps versions instantly.
            </p>
          </div>
        </div>

        <div class="mt-12 grid lg:grid-cols-[1.3fr_1fr] gap-8">
          <div class="p-6 rounded-2xl bg-gradient-to-br from-slate-900/80 to-slate-800/60 border border-slate-700/60">
            <div class="flex items-center justify-between mb-4">
              <h3 class="text-xl font-semibold">Popular runtimes</h3>
              <span class="text-sm text-slate-500">+100 more via mise</span>
            </div>
            <div class="flex flex-wrap gap-2">
              <For each={RUNTIME_TAGS}>
                {(tag) => (
                  <span class="px-3 py-1 rounded-full text-xs font-mono bg-slate-800 text-slate-300 border border-slate-700/60">
                    {tag}
                  </span>
                )}
              </For>
            </div>
          </div>

          <div class="p-6 rounded-2xl bg-slate-900 border border-slate-700/60">
            <div class="text-sm text-slate-400 mb-3">Runtime switches in practice</div>
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
