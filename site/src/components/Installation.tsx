import { Component, createSignal } from 'solid-js';

const Installation: Component = () => {
  const [copied, setCopied] = createSignal(false);
  const [activeTab, setActiveTab] = createSignal<'curl' | 'arch' | 'cargo'>('curl');
  
  const commands = {
    curl: 'curl -fsSL https://pyro1121.com/install.sh | bash',
    arch: 'yay -S omg-bin',
    cargo: 'cargo install omg-cli',
  };

  const copyToClipboard = () => {
    navigator.clipboard.writeText(commands[activeTab()]);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <section id="install" class="py-32 px-6 relative">
      {/* Background */}
      <div class="absolute inset-0 bg-gradient-to-b from-slate-900 via-indigo-950/50 to-slate-900 pointer-events-none" />
      
      <div class="max-w-5xl mx-auto relative">
        {/* Header */}
        <div class="text-center mb-16">
          <div class="inline-flex items-center gap-2 px-4 py-2 rounded-full bg-green-500/10 border border-green-500/20 text-sm text-green-300 mb-6">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
            </svg>
            <span>Quick Install</span>
          </div>
          <h2 class="text-4xl md:text-5xl lg:text-6xl font-bold mb-6">
            Up and Running in <span class="text-green-400">10 Seconds</span>
          </h2>
          <p class="text-xl text-slate-400 max-w-2xl mx-auto">
            One command installs OMG with zero dependencies. Works on Arch, Debian, Ubuntu, and any Linux distro.
          </p>
        </div>

        {/* Install tabs */}
        <div class="max-w-3xl mx-auto">
          <div class="flex justify-center gap-2 mb-6">
            <button 
              onClick={() => setActiveTab('curl')}
              class={`px-4 py-2 rounded-lg text-sm font-medium transition-all ${
                activeTab() === 'curl' 
                  ? 'bg-indigo-500 text-white' 
                  : 'bg-slate-800 text-slate-400 hover:text-white'
              }`}
            >
              Quick Install
            </button>
            <button 
              onClick={() => setActiveTab('arch')}
              class={`px-4 py-2 rounded-lg text-sm font-medium transition-all ${
                activeTab() === 'arch' 
                  ? 'bg-indigo-500 text-white' 
                  : 'bg-slate-800 text-slate-400 hover:text-white'
              }`}
            >
              Arch Linux
            </button>
            <button 
              onClick={() => setActiveTab('cargo')}
              class={`px-4 py-2 rounded-lg text-sm font-medium transition-all ${
                activeTab() === 'cargo' 
                  ? 'bg-indigo-500 text-white' 
                  : 'bg-slate-800 text-slate-400 hover:text-white'
              }`}
            >
              Cargo
            </button>
          </div>

          {/* Command box */}
          <div class="terminal glow-strong mb-8">
            <div class="terminal-header">
              <div class="terminal-dot red" />
              <div class="terminal-dot yellow" />
              <div class="terminal-dot green" />
              <span class="ml-4 text-xs text-slate-500 font-mono">terminal</span>
            </div>
            <div class="terminal-body flex items-center justify-between gap-4">
              <code class="text-base md:text-lg font-mono flex-1 overflow-x-auto">
                <span class="terminal-prompt">$ </span>
                <span class="terminal-command">{commands[activeTab()]}</span>
              </code>
              <button 
                onClick={copyToClipboard}
                class="flex-shrink-0 p-3 rounded-xl bg-indigo-500/20 hover:bg-indigo-500/30 border border-indigo-500/30 transition-all group"
                title="Copy to clipboard"
              >
                {copied() ? (
                  <svg class="w-5 h-5 text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                  </svg>
                ) : (
                  <svg class="w-5 h-5 text-indigo-400 group-hover:text-indigo-300" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                  </svg>
                )}
              </button>
            </div>
          </div>

          {/* What happens next */}
          <div class="grid md:grid-cols-3 gap-4 mb-12">
            <div class="flex items-start gap-3 p-4 rounded-xl bg-slate-800/50 border border-slate-700/50">
              <div class="w-8 h-8 rounded-full bg-indigo-500/20 flex items-center justify-center flex-shrink-0">
                <span class="text-indigo-400 font-bold text-sm">1</span>
              </div>
              <div>
                <h4 class="font-medium text-sm mb-1">Downloads binary</h4>
                <p class="text-xs text-slate-500">Pre-compiled for your architecture</p>
              </div>
            </div>
            <div class="flex items-start gap-3 p-4 rounded-xl bg-slate-800/50 border border-slate-700/50">
              <div class="w-8 h-8 rounded-full bg-indigo-500/20 flex items-center justify-center flex-shrink-0">
                <span class="text-indigo-400 font-bold text-sm">2</span>
              </div>
              <div>
                <h4 class="font-medium text-sm mb-1">Installs to ~/.local/bin</h4>
                <p class="text-xs text-slate-500">No sudo required</p>
              </div>
            </div>
            <div class="flex items-start gap-3 p-4 rounded-xl bg-slate-800/50 border border-slate-700/50">
              <div class="w-8 h-8 rounded-full bg-green-500/20 flex items-center justify-center flex-shrink-0">
                <span class="text-green-400 font-bold text-sm">✓</span>
              </div>
              <div>
                <h4 class="font-medium text-sm mb-1">Ready to use</h4>
                <p class="text-xs text-slate-500">Run `omg` immediately</p>
              </div>
            </div>
          </div>
        </div>

        {/* Platform cards */}
        <div class="grid md:grid-cols-3 gap-6 max-w-4xl mx-auto">
          <div class="p-6 rounded-2xl bg-gradient-to-br from-cyan-500/10 to-blue-500/10 border border-cyan-500/20 hover:border-cyan-500/40 transition-colors">
            <div class="w-12 h-12 rounded-xl bg-cyan-500/20 flex items-center justify-center mb-4">
              <svg class="w-6 h-6 text-cyan-400" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z"/>
              </svg>
            </div>
            <h3 class="font-bold text-lg mb-2">Arch Linux</h3>
            <p class="text-slate-400 text-sm mb-3">Native pacman + AUR with direct libalpm bindings. 22x faster than pacman.</p>
            <code class="text-xs text-cyan-400 font-mono">yay -S omg-bin</code>
          </div>

          <div class="p-6 rounded-2xl bg-gradient-to-br from-orange-500/10 to-red-500/10 border border-orange-500/20 hover:border-orange-500/40 transition-colors">
            <div class="w-12 h-12 rounded-xl bg-orange-500/20 flex items-center justify-center mb-4">
              <svg class="w-6 h-6 text-orange-400" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z"/>
              </svg>
            </div>
            <h3 class="font-bold text-lg mb-2">Debian / Ubuntu</h3>
            <p class="text-slate-400 text-sm mb-3">Full APT integration via rust-apt. Up to 300x faster than apt.</p>
            <code class="text-xs text-orange-400 font-mono">curl ... | bash</code>
          </div>

          <div class="p-6 rounded-2xl bg-gradient-to-br from-purple-500/10 to-pink-500/10 border border-purple-500/20 hover:border-purple-500/40 transition-colors">
            <div class="w-12 h-12 rounded-xl bg-purple-500/20 flex items-center justify-center mb-4">
              <svg class="w-6 h-6 text-purple-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" />
              </svg>
            </div>
            <h3 class="font-bold text-lg mb-2">Build from Source</h3>
            <p class="text-slate-400 text-sm mb-3">100% Rust, compiles anywhere. Optimized with LTO for maximum speed.</p>
            <code class="text-xs text-purple-400 font-mono">cargo install omg-cli</code>
          </div>
        </div>

        {/* Shell hook */}
        <div class="mt-16 text-center">
          <p class="text-slate-400 mb-4">Enable instant version switching with the shell hook:</p>
          <div class="terminal max-w-lg mx-auto">
            <div class="terminal-body text-left text-sm">
              <div class="text-slate-500 mb-2"># Add to ~/.zshrc or ~/.bashrc</div>
              <div class="text-cyan-400">eval "$(omg hook zsh)"</div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};

export default Installation;
