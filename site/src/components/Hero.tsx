import { Component, createSignal, onMount } from 'solid-js';

const Hero: Component = () => {
  const [typedText, setTypedText] = createSignal('');
  const [showCursor, setShowCursor] = createSignal(true);

  const commands = [
    {
      cmd: 'omg search firefox',
      output: 'Found 12 packages in 6ms',
      highlight: '(pacman: 133ms, yay: 150ms)',
    },
    { cmd: 'omg use node 22', output: '✓ Switched to node v22.0.0', highlight: '(1.8ms)' },
    { cmd: 'omg audit', output: '✓ No vulnerabilities found', highlight: '(scanned 847 packages)' },
  ];

  onMount(() => {
    let cmdIndex = 0;
    let charIndex = 0;
    let isTyping = true;

    const interval = setInterval(() => {
      if (isTyping) {
        const cmd = commands[cmdIndex].cmd;
        if (charIndex < cmd.length) {
          setTypedText(cmd.slice(0, charIndex + 1));
          charIndex++;
        } else {
          isTyping = false;
          setTimeout(() => {
            charIndex = 0;
            cmdIndex = (cmdIndex + 1) % commands.length;
            isTyping = true;
          }, 2000);
        }
      }
    }, 80);

    const cursorInterval = setInterval(() => setShowCursor(v => !v), 530);

    return () => {
      clearInterval(interval);
      clearInterval(cursorInterval);
    };
  });

  return (
    <section class="relative flex min-h-screen items-center overflow-hidden px-6 pt-24 pb-20">
      {/* Animated background */}
      <div class="pointer-events-none absolute inset-0 overflow-hidden">
        <div class="bg-gradient-radial animate-pulse-slow absolute -top-1/2 -left-1/2 h-full w-full from-indigo-500/20 via-transparent to-transparent" />
        <div class="bg-gradient-radial animate-pulse-slow absolute -right-1/2 -bottom-1/2 h-full w-full from-cyan-500/15 via-transparent to-transparent delay-1000" />
        <div class="absolute inset-0 bg-[url('data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNjAiIGhlaWdodD0iNjAiIHZpZXdCb3g9IjAgMCA2MCA2MCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48ZyBmaWxsPSJub25lIiBmaWxsLXJ1bGU9ImV2ZW5vZGQiPjxnIGZpbGw9IiM2MzY2ZjEiIGZpbGwtb3BhY2l0eT0iMC4wMyI+PGNpcmNsZSBjeD0iMzAiIGN5PSIzMCIgcj0iMiIvPjwvZz48L2c+PC9zdmc+')] opacity-50" />
      </div>

      <div class="relative mx-auto w-full max-w-7xl">
        <div class="grid items-center gap-16 lg:grid-cols-2">
          {/* Left: Copy */}
          <div class="text-left">
            {/* Badge */}
            <div class="mb-8 inline-flex items-center gap-2 rounded-full border border-indigo-500/30 bg-gradient-to-r from-indigo-500/20 to-cyan-500/20 px-4 py-2 text-sm backdrop-blur-sm">
              <span class="relative flex h-2 w-2">
                <span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-green-400 opacity-75" />
                <span class="relative inline-flex h-2 w-2 rounded-full bg-green-500" />
              </span>
              <span class="text-slate-300">Native runtimes • 100+ via mise • Pure Rust</span>
            </div>

            {/* Main headline - SEO optimized with keywords */}
            <h1 class="mb-6 text-5xl leading-[1.1] font-black tracking-tight md:text-6xl lg:text-7xl">
              <span class="text-white">The Fastest </span>
              <span class="gradient-text">Package Manager</span>
              <br />
              <span class="text-white">for </span>
              <span class="text-cyan-400">Linux</span>
            </h1>

            <p class="mb-8 max-w-xl text-xl leading-relaxed text-slate-400 md:text-2xl">
              Native managers for <span class="font-medium text-white">Node</span>,{' '}
              <span class="font-medium text-white">Python</span>,{' '}
              <span class="font-medium text-white">Go</span>,{' '}
              <span class="font-medium text-white">Rust</span>,{' '}
              <span class="font-medium text-white">Ruby</span>,{' '}
              <span class="font-medium text-white">Java</span>, and{' '}
              <span class="font-medium text-white">Bun</span>. The long‑tail of 100+ runtimes comes
              from <span class="font-medium text-white">mise</span> while we keep expanding the
              native core.
              <span class="font-semibold text-cyan-400"> 22x faster</span> than pacman/yay.
            </p>

            {/* CTA buttons */}
            <div class="mb-12 flex flex-col items-start gap-4 sm:flex-row">
              <a href="#install" class="btn-primary group text-lg">
                <svg
                  class="h-5 w-5 transition-transform group-hover:-translate-y-0.5"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"
                  />
                </svg>
                Install in 10 Seconds
              </a>
              <a href="#features" class="btn-secondary group text-lg">
                See How It Works
                <svg
                  class="h-5 w-5 transition-transform group-hover:translate-x-1"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M17 8l4 4m0 0l-4 4m4-4H3"
                  />
                </svg>
              </a>
            </div>

            {/* Trust badges */}
            <div class="flex flex-wrap items-center gap-6 text-sm text-slate-500">
              <div class="flex items-center gap-2">
                <svg class="h-5 w-5 text-green-500" fill="currentColor" viewBox="0 0 20 20">
                  <path
                    fill-rule="evenodd"
                    d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
                    clip-rule="evenodd"
                  />
                </svg>
                <span>No sudo required</span>
              </div>
              <div class="flex items-center gap-2">
                <svg class="h-5 w-5 text-green-500" fill="currentColor" viewBox="0 0 20 20">
                  <path
                    fill-rule="evenodd"
                    d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
                    clip-rule="evenodd"
                  />
                </svg>
                <span>Works offline</span>
              </div>
              <div class="flex items-center gap-2">
                <svg class="h-5 w-5 text-green-500" fill="currentColor" viewBox="0 0 20 20">
                  <path
                    fill-rule="evenodd"
                    d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
                    clip-rule="evenodd"
                  />
                </svg>
                <span>Arch • Debian • Ubuntu</span>
              </div>
            </div>
          </div>

          {/* Right: Terminal */}
          <div class="relative">
            <div class="terminal glow-strong">
              <div class="terminal-header">
                <div class="terminal-dot red" />
                <div class="terminal-dot yellow" />
                <div class="terminal-dot green" />
                <span class="ml-4 font-mono text-xs text-slate-500">~</span>
              </div>
              <div class="terminal-body text-left font-mono">
                <div class="mb-4">
                  <span class="terminal-prompt">❯ </span>
                  <span class="terminal-command">{typedText()}</span>
                  <span class={`${showCursor() ? 'opacity-100' : 'opacity-0'} text-cyan-400`}>
                    ▋
                  </span>
                </div>
                <div class="space-y-3 text-sm">
                  <div class="flex items-center gap-2">
                    <span class="text-green-400">✓</span>
                    <span class="text-slate-300">Found 12 packages in</span>
                    <span class="font-bold text-cyan-400">6ms</span>
                  </div>
                  <div class="pl-5 text-xs text-slate-500">
                    vs pacman: 133ms • yay: 150ms • apt: 652ms
                  </div>
                </div>

                <div class="mt-6 border-t border-slate-700/50 pt-4">
                  <div class="grid grid-cols-3 gap-4 text-center">
                    <div>
                      <div class="text-2xl font-bold text-cyan-400">6ms</div>
                      <div class="text-xs text-slate-500">Query Time</div>
                    </div>
                    <div>
                      <div class="text-2xl font-bold text-indigo-400">1.8ms</div>
                      <div class="text-xs text-slate-500">Version Switch</div>
                    </div>
                    <div>
                      <div class="text-2xl font-bold text-green-400">0</div>
                      <div class="text-xs text-slate-500">Dependencies</div>
                    </div>
                  </div>
                </div>
              </div>
            </div>

            {/* Floating badges */}
            <div class="animate-bounce-slow absolute -top-4 -right-4 rounded-full bg-gradient-to-r from-green-500 to-emerald-500 px-3 py-1.5 text-xs font-bold text-white shadow-lg">
              22x Faster
            </div>
            <div class="absolute -bottom-4 -left-4 rounded-full bg-gradient-to-r from-indigo-500 to-purple-500 px-3 py-1.5 text-xs font-bold text-white shadow-lg">
              Pure Rust
            </div>
          </div>
        </div>

        {/* Stats bar */}
        <div class="mt-20 border-t border-slate-800 pt-12">
          <div class="grid grid-cols-2 gap-8 md:grid-cols-4">
            <div class="group text-center">
              <div class="stat-number transition-transform group-hover:scale-110">6ms</div>
              <div class="text-sm text-slate-400">Average Query</div>
            </div>
            <div class="group text-center">
              <div class="stat-number transition-transform group-hover:scale-110">100+</div>
              <div class="text-sm text-slate-400">Runtimes via mise</div>
            </div>
            <div class="group text-center">
              <div class="stat-number transition-transform group-hover:scale-110">22x</div>
              <div class="text-sm text-slate-400">Faster Than pacman</div>
            </div>
            <div class="group text-center">
              <div class="stat-number transition-transform group-hover:scale-110">0</div>
              <div class="text-sm text-slate-400">Runtime Dependencies</div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};

export default Hero;
