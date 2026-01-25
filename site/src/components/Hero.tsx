import { Component } from 'solid-js';
import HeroTerminal from './hero/HeroTerminal';

const Hero: Component = () => {
  return (
    <section class="relative flex min-h-screen items-center overflow-hidden px-6 pt-24 pb-20">
      {/* Background decoration moved to BackgroundMesh or kept subtle here */}
      <div class="pointer-events-none absolute inset-0 overflow-hidden">
        <div class="bg-gradient-radial animate-pulse-slow absolute -top-1/2 -left-1/2 h-full w-full from-indigo-500/10 via-transparent to-transparent" />
      </div>

      <div class="relative mx-auto w-full max-w-7xl">
        <div class="grid items-center gap-16 lg:grid-cols-2">
          {/* Left: Copy */}
          <div class="text-left">
            {/* Badge */}
            <div class="animate-fade-in-up mb-8 inline-flex items-center gap-2 rounded-full border border-indigo-500/30 bg-gradient-to-r from-indigo-500/20 to-cyan-500/20 px-4 py-2 text-sm backdrop-blur-sm">
              <span class="relative flex h-2 w-2">
                <span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-green-400 opacity-75" />
                <span class="relative inline-flex h-2 w-2 rounded-full bg-green-500" />
              </span>
              <span class="text-slate-300">Native runtimes • 100+ via mise • Pure Rust</span>
            </div>

            {/* Main headline */}
            <h1 class="animate-fade-in-up delay-100 mb-6 text-5xl leading-[1.1] font-black tracking-tight md:text-6xl lg:text-7xl">
              <span class="text-white">The Fastest </span>
              <span class="gradient-text">Package Manager</span>
              <br />
              <span class="text-white">for </span>
              <span class="text-cyan-400">Linux</span>
            </h1>

            <p class="animate-fade-in-up delay-200 mb-8 max-w-xl text-xl leading-relaxed text-slate-400 md:text-2xl">
              Native managers for <span class="font-medium text-white">Node, Python, Go, Rust</span>, and more. 
              Built in Rust for extreme performance.
              <span class="font-semibold text-cyan-400"> 22x faster</span> than standard tools.
            </p>

            {/* CTA buttons */}
            <div class="animate-fade-in-up delay-300 mb-12 flex flex-col items-start gap-4 sm:flex-row">
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
            <div class="animate-fade-in-up delay-400 flex flex-wrap items-center gap-6 text-sm text-slate-500">
              <div class="flex items-center gap-2">
                <svg class="h-5 w-5 text-green-500" fill="currentColor" viewBox="0 0 20 20">
                  <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
                </svg>
                <span>No sudo required</span>
              </div>
              <div class="flex items-center gap-2">
                <svg class="h-5 w-5 text-green-500" fill="currentColor" viewBox="0 0 20 20">
                  <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
                </svg>
                <span>Arch • Debian • Ubuntu</span>
              </div>
            </div>
          </div>

          {/* Right: The 3D Glass Terminal */}
          <div class="animate-fade-in-up delay-500 relative">
            <HeroTerminal />
            
            {/* Floating badges around terminal */}
            <div class="animate-bounce-slow absolute -top-6 -right-6 rounded-full bg-gradient-to-r from-green-500 to-emerald-500 px-4 py-2 text-xs font-bold text-white shadow-xl z-10">
              22x Faster
            </div>
            <div class="absolute -bottom-6 -left-6 rounded-full bg-gradient-to-r from-indigo-500 to-purple-500 px-4 py-2 text-xs font-bold text-white shadow-xl z-10">
              Pure Rust
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};

export default Hero;
