import { Component } from 'solid-js';

const Footer: Component = () => {
  return (
    <footer class="border-t border-white/5 px-6 py-12">
      <div class="mx-auto max-w-7xl">
        <div class="mb-12 grid gap-8 md:grid-cols-4">
          <div>
            <div class="mb-4 flex items-center gap-3">
              <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500 to-cyan-400 text-lg font-bold">
                ⚡
              </div>
              <span class="text-xl font-bold">OMG</span>
            </div>
            <p class="text-sm text-slate-400">
              The fastest unified package manager for Linux. Built with pure Rust.
            </p>
          </div>

          <div>
            <h4 class="mb-4 font-semibold">Product</h4>
            <ul class="space-y-2 text-sm text-slate-400">
              <li>
                <a href="#features" class="transition-colors hover:text-white">
                  Features
                </a>
              </li>
              <li>
                <a href="#benchmarks" class="transition-colors hover:text-white">
                  Benchmarks
                </a>
              </li>
              <li>
                <a href="#pricing" class="transition-colors hover:text-white">
                  Pricing
                </a>
              </li>
              <li>
                <a href="#install" class="transition-colors hover:text-white">
                  Installation
                </a>
              </li>
            </ul>
          </div>

          <div>
            <h4 class="mb-4 font-semibold">Resources</h4>
            <ul class="space-y-2 text-sm text-slate-400">
              <li>
                <a
                  href="https://github.com/PyRo1121/omg/"
                  class="transition-colors hover:text-white"
                >
                  GitHub
                </a>
              </li>
              <li>
                <a
                  href="https://github.com/PyRo1121/omg/blob/main/README.md"
                  class="transition-colors hover:text-white"
                >
                  Documentation
                </a>
              </li>
              <li>
                <a
                  href="https://github.com/PyRo1121/omg/releases"
                  class="transition-colors hover:text-white"
                >
                  Releases
                </a>
              </li>
              <li>
                <a
                  href="https://github.com/PyRo1121/omg/issues"
                  class="transition-colors hover:text-white"
                >
                  Issues
                </a>
              </li>
            </ul>
          </div>

          <div>
            <h4 class="mb-4 font-semibold">Legal</h4>
            <ul class="space-y-2 text-sm text-slate-400">
              <li>
                <a href="/privacy" class="transition-colors hover:text-white">
                  Privacy Policy
                </a>
              </li>
              <li>
                <a href="/terms" class="transition-colors hover:text-white">
                  Terms of Service
                </a>
              </li>
              <li>
                <a href="mailto:support@pyro1121.com" class="transition-colors hover:text-white">
                  Contact
                </a>
              </li>
            </ul>
          </div>
        </div>

        <div class="flex flex-col items-center justify-between gap-4 border-t border-white/5 pt-8 md:flex-row">
          <p class="text-sm text-slate-500">AGPL-3.0-or-later © 2026 OMG Team</p>
          <div class="flex items-center gap-6">
            <a
              href="https://github.com/PyRo1121/omg"
              class="text-slate-400 transition-colors hover:text-white"
              aria-label="OMG Package Manager on GitHub"
              rel="noopener noreferrer"
              target="_blank"
            >
              <svg class="h-6 w-6" fill="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z" />
              </svg>
            </a>
            <a
              href="https://twitter.com/pyro1121"
              class="text-slate-400 transition-colors hover:text-white"
              aria-label="OMG Package Manager on X (Twitter)"
              rel="noopener noreferrer"
              target="_blank"
            >
              <svg class="h-6 w-6" fill="currentColor" viewBox="0 0 24 24" aria-hidden="true">
                <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z" />
              </svg>
            </a>
          </div>
        </div>
      </div>
    </footer>
  );
};

export default Footer;
