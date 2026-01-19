import { Component } from 'solid-js';

const Benchmarks: Component = () => {
  return (
    <section
      id="benchmarks"
      class="bg-gradient-to-b from-transparent via-indigo-500/5 to-transparent px-6 py-24"
    >
      <div class="mx-auto max-w-7xl">
        <div class="mb-16 text-center">
          <h2 class="mb-4 text-4xl font-bold md:text-5xl">
            Real-World <span class="gradient-text">Performance</span>
          </h2>
          <p class="mx-auto max-w-2xl text-xl text-slate-400">
            Benchmarked on Intel i9-14900K with 10 iterations. These aren't synthetic tests—this is
            real package management.
          </p>
        </div>

        <div class="grid gap-8 lg:grid-cols-2">
          {/* Arch Linux */}
          <div class="gradient-border p-8">
            <h3 class="mb-6 flex items-center gap-3 text-2xl font-bold">
              <span class="text-3xl">🐧</span>
              Arch Linux (pacman/yay)
            </h3>

            <div class="space-y-4">
              <div class="benchmark-row rounded-lg bg-indigo-500/10">
                <span class="font-medium">Command</span>
                <span class="font-mono text-cyan-400">OMG</span>
                <span class="font-mono text-slate-400">pacman</span>
                <span class="font-semibold text-green-400">Speedup</span>
              </div>

              <div class="benchmark-row">
                <span>search</span>
                <span class="font-mono font-bold text-cyan-400">6ms</span>
                <span class="font-mono text-slate-400">133ms</span>
                <span class="font-semibold text-green-400">22x</span>
              </div>

              <div class="benchmark-row">
                <span>info</span>
                <span class="font-mono font-bold text-cyan-400">6.5ms</span>
                <span class="font-mono text-slate-400">138ms</span>
                <span class="font-semibold text-green-400">21x</span>
              </div>

              <div class="benchmark-row">
                <span>explicit</span>
                <span class="font-mono font-bold text-cyan-400">1.2ms</span>
                <span class="font-mono text-slate-400">14ms</span>
                <span class="font-semibold text-green-400">12x</span>
              </div>
            </div>
          </div>

          {/* Debian/Ubuntu */}
          <div class="gradient-border p-8">
            <h3 class="mb-6 flex items-center gap-3 text-2xl font-bold">
              <span class="text-3xl">🍥</span>
              Debian/Ubuntu (apt)
            </h3>

            <div class="space-y-4">
              <div class="benchmark-row rounded-lg bg-indigo-500/10">
                <span class="font-medium">Command</span>
                <span class="font-mono text-cyan-400">OMG</span>
                <span class="font-mono text-slate-400">apt-cache</span>
                <span class="font-semibold text-green-400">Speedup</span>
              </div>

              <div class="benchmark-row">
                <span>search</span>
                <span class="font-mono font-bold text-cyan-400">11ms</span>
                <span class="font-mono text-slate-400">652ms</span>
                <span class="font-semibold text-green-400">59x</span>
              </div>

              <div class="benchmark-row">
                <span>info</span>
                <span class="font-mono font-bold text-cyan-400">27ms</span>
                <span class="font-mono text-slate-400">462ms</span>
                <span class="font-semibold text-green-400">17x</span>
              </div>

              <div class="benchmark-row">
                <span>explicit</span>
                <span class="font-mono font-bold text-cyan-400">2ms</span>
                <span class="font-mono text-slate-400">601ms</span>
                <span class="font-semibold text-green-400">300x</span>
              </div>
            </div>
          </div>
        </div>

        {/* Runtime switching */}
        <div class="gradient-border mt-12 p-8">
          <h3 class="mb-6 text-center text-2xl font-bold">Runtime Version Switching</h3>

          <div class="grid gap-8 text-center md:grid-cols-4">
            <div>
              <div class="mb-2 text-4xl font-bold text-cyan-400">1.8ms</div>
              <div class="text-slate-400">OMG</div>
            </div>
            <div>
              <div class="mb-2 text-4xl font-bold text-slate-500">150ms</div>
              <div class="text-slate-400">nvm</div>
            </div>
            <div>
              <div class="mb-2 text-4xl font-bold text-slate-500">200ms</div>
              <div class="text-slate-400">pyenv</div>
            </div>
            <div>
              <div class="mb-2 text-4xl font-bold text-green-400">83-111x</div>
              <div class="text-slate-400">Faster</div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};

export default Benchmarks;
