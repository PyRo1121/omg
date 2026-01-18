import { Component } from 'solid-js';

const Benchmarks: Component = () => {
  return (
    <section id="benchmarks" class="py-24 px-6 bg-gradient-to-b from-transparent via-indigo-500/5 to-transparent">
      <div class="max-w-7xl mx-auto">
        <div class="text-center mb-16">
          <h2 class="text-4xl md:text-5xl font-bold mb-4">
            Real-World <span class="gradient-text">Performance</span>
          </h2>
          <p class="text-xl text-slate-400 max-w-2xl mx-auto">
            Benchmarked on Intel i9-14900K with 10 iterations. These aren't synthetic tests—this is real package management.
          </p>
        </div>

        <div class="grid lg:grid-cols-2 gap-8">
          {/* Arch Linux */}
          <div class="gradient-border p-8">
            <h3 class="text-2xl font-bold mb-6 flex items-center gap-3">
              <span class="text-3xl">🐧</span>
              Arch Linux (pacman/yay)
            </h3>
            
            <div class="space-y-4">
              <div class="benchmark-row bg-indigo-500/10 rounded-lg">
                <span class="font-medium">Command</span>
                <span class="text-cyan-400 font-mono">OMG</span>
                <span class="text-slate-400 font-mono">pacman</span>
                <span class="text-green-400 font-semibold">Speedup</span>
              </div>
              
              <div class="benchmark-row">
                <span>search</span>
                <span class="text-cyan-400 font-mono font-bold">6ms</span>
                <span class="text-slate-400 font-mono">133ms</span>
                <span class="text-green-400 font-semibold">22x</span>
              </div>
              
              <div class="benchmark-row">
                <span>info</span>
                <span class="text-cyan-400 font-mono font-bold">6.5ms</span>
                <span class="text-slate-400 font-mono">138ms</span>
                <span class="text-green-400 font-semibold">21x</span>
              </div>
              
              <div class="benchmark-row">
                <span>explicit</span>
                <span class="text-cyan-400 font-mono font-bold">1.2ms</span>
                <span class="text-slate-400 font-mono">14ms</span>
                <span class="text-green-400 font-semibold">12x</span>
              </div>
            </div>
          </div>

          {/* Debian/Ubuntu */}
          <div class="gradient-border p-8">
            <h3 class="text-2xl font-bold mb-6 flex items-center gap-3">
              <span class="text-3xl">🍥</span>
              Debian/Ubuntu (apt)
            </h3>
            
            <div class="space-y-4">
              <div class="benchmark-row bg-indigo-500/10 rounded-lg">
                <span class="font-medium">Command</span>
                <span class="text-cyan-400 font-mono">OMG</span>
                <span class="text-slate-400 font-mono">apt-cache</span>
                <span class="text-green-400 font-semibold">Speedup</span>
              </div>
              
              <div class="benchmark-row">
                <span>search</span>
                <span class="text-cyan-400 font-mono font-bold">11ms</span>
                <span class="text-slate-400 font-mono">652ms</span>
                <span class="text-green-400 font-semibold">59x</span>
              </div>
              
              <div class="benchmark-row">
                <span>info</span>
                <span class="text-cyan-400 font-mono font-bold">27ms</span>
                <span class="text-slate-400 font-mono">462ms</span>
                <span class="text-green-400 font-semibold">17x</span>
              </div>
              
              <div class="benchmark-row">
                <span>explicit</span>
                <span class="text-cyan-400 font-mono font-bold">2ms</span>
                <span class="text-slate-400 font-mono">601ms</span>
                <span class="text-green-400 font-semibold">300x</span>
              </div>
            </div>
          </div>
        </div>

        {/* Runtime switching */}
        <div class="mt-12 gradient-border p-8">
          <h3 class="text-2xl font-bold mb-6 text-center">
            Runtime Version Switching
          </h3>
          
          <div class="grid md:grid-cols-4 gap-8 text-center">
            <div>
              <div class="text-4xl font-bold text-cyan-400 mb-2">1.8ms</div>
              <div class="text-slate-400">OMG</div>
            </div>
            <div>
              <div class="text-4xl font-bold text-slate-500 mb-2">150ms</div>
              <div class="text-slate-400">nvm</div>
            </div>
            <div>
              <div class="text-4xl font-bold text-slate-500 mb-2">200ms</div>
              <div class="text-slate-400">pyenv</div>
            </div>
            <div>
              <div class="text-4xl font-bold text-green-400 mb-2">83-111x</div>
              <div class="text-slate-400">Faster</div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};

export default Benchmarks;
