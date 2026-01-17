import { Component, createSignal, onMount, Show, For } from 'solid-js';
import Header from './components/Header';
import Hero from './components/Hero';
import Features from './components/Features';
import RuntimeEcosystem from './components/RuntimeEcosystem';
import Benchmarks from './components/Benchmarks';
import Pricing from './components/Pricing';
import Installation from './components/Installation';
import Footer from './components/Footer';

const CONFETTI_COLORS = ['#6366f1', '#8b5cf6', '#ec4899', '#10b981', '#f59e0b', '#3b82f6'];

const App: Component = () => {
  const [showSuccess, setShowSuccess] = createSignal(false);
  const [licenseKey, setLicenseKey] = createSignal<string | null>(null);
  const [tier, setTier] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [email, setEmail] = createSignal('');
  const [copied, setCopied] = createSignal(false);
  const [confetti, setConfetti] = createSignal<Array<{ id: number; left: number; color: string; delay: number }>>([]);
  const [notFound, setNotFound] = createSignal(false);
  const [retryCount, setRetryCount] = createSignal(0);

  const spawnConfetti = () => {
    const pieces = Array.from({ length: 50 }, (_, i) => ({
      id: i,
      left: Math.random() * 100,
      color: CONFETTI_COLORS[Math.floor(Math.random() * CONFETTI_COLORS.length)],
      delay: Math.random() * 0.5,
    }));
    setConfetti(pieces);
    setTimeout(() => setConfetti([]), 4000);
  };

  onMount(() => {
    const params = new URLSearchParams(window.location.search);
    if (params.get('success') === 'true') {
      setShowSuccess(true);
      spawnConfetti();
      window.history.replaceState({}, '', '/');
    }
  });

  const fetchLicense = async () => {
    const userEmail = email();
    if (!userEmail) return;
    
    setLoading(true);
    setNotFound(false);
    
    try {
      const res = await fetch(`https://api.pyro1121.com/api/get-license?email=${encodeURIComponent(userEmail)}`);
      const data = await res.json();
      if (data.found) {
        setLicenseKey(data.license_key);
        setTier(data.tier);
        spawnConfetti();
      } else {
        setNotFound(true);
        setRetryCount(c => c + 1);
      }
    } catch (e) {
      console.error(e);
      setNotFound(true);
    }
    setLoading(false);
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleClose = () => {
    setShowSuccess(false);
    setLicenseKey(null);
    setTier(null);
    setEmail('');
    setNotFound(false);
    setRetryCount(0);
  };

  return (
    <div class="min-h-screen">
      <Header />
      <main>
        <Hero />
        <Features />
        <RuntimeEcosystem />
        <Benchmarks />
        <Installation />
        <Pricing />
      </main>
      <Footer />

      {/* Confetti */}
      <For each={confetti()}>
        {(piece) => (
          <div
            class="confetti-piece rounded-sm"
            style={{
              left: `${piece.left}%`,
              background: piece.color,
              'animation-delay': `${piece.delay}s`,
            }}
          />
        )}
      </For>

      {/* Success Modal */}
      <Show when={showSuccess()}>
        <div 
          class="fixed inset-0 z-50 flex items-center justify-center p-4"
          onClick={(e) => e.target === e.currentTarget && handleClose()}
        >
          <div class="absolute inset-0 bg-black/80 backdrop-blur-md animate-fade-in" />
          
          <div class="relative bg-gradient-to-b from-slate-800 to-slate-900 border border-slate-700/50 rounded-3xl max-w-lg w-full shadow-2xl shadow-black/50 animate-scale-in overflow-hidden">
            {/* Glow effects */}
            <div class="absolute -top-20 -right-20 w-40 h-40 bg-green-500/20 rounded-full blur-3xl pointer-events-none" />
            <div class="absolute -bottom-20 -left-20 w-40 h-40 bg-indigo-500/20 rounded-full blur-3xl pointer-events-none" />

            <div class="relative p-8">
              {/* Header */}
              <div class="text-center mb-8">
                <div class="relative w-20 h-20 mx-auto mb-6">
                  <div class="absolute inset-0 bg-green-500/20 rounded-full animate-ping" style={{ 'animation-duration': '2s' }} />
                  <div class="relative w-full h-full bg-gradient-to-br from-green-400 to-emerald-500 rounded-full flex items-center justify-center shadow-lg shadow-green-500/30">
                    <svg class="w-10 h-10 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
                    </svg>
                  </div>
                </div>
                <h2 class="text-3xl font-bold text-white mb-2">Welcome to OMG Pro!</h2>
                <p class="text-slate-400">Your payment was successful. Let's get you set up.</p>
              </div>

              <Show when={!licenseKey()}>
                <div class="space-y-5">
                  <div class="bg-slate-800/50 border border-slate-700/50 rounded-2xl p-5">
                    <div class="flex items-start gap-3 mb-4">
                      <div class="p-2 bg-indigo-500/20 rounded-lg">
                        <svg class="w-5 h-5 text-indigo-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
                        </svg>
                      </div>
                      <div>
                        <p class="text-white font-medium">Retrieve Your License</p>
                        <p class="text-sm text-slate-400">Enter the email you used at checkout</p>
                      </div>
                    </div>
                    
                    <input
                      type="email"
                      value={email()}
                      onInput={(e) => setEmail(e.currentTarget.value)}
                      onKeyPress={(e) => e.key === 'Enter' && fetchLicense()}
                      placeholder="you@company.com"
                      class="w-full px-4 py-3 bg-slate-900/50 border border-slate-600 rounded-xl text-white placeholder-slate-500 focus:outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 transition-all"
                    />
                  </div>

                  <Show when={notFound()}>
                    <div class="flex items-start gap-3 p-4 bg-amber-500/10 border border-amber-500/30 rounded-xl">
                      <svg class="w-5 h-5 text-amber-400 flex-shrink-0 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                      </svg>
                      <div>
                        <p class="text-amber-400 font-medium text-sm">License not found yet</p>
                        <p class="text-amber-400/70 text-xs mt-1">
                          {retryCount() < 3 
                            ? "It may take a moment to process. Please try again in a few seconds."
                            : "Still processing? Check your email for confirmation or contact support."}
                        </p>
                      </div>
                    </div>
                  </Show>

                  <button
                    onClick={fetchLicense}
                    disabled={loading() || !email()}
                    class="w-full py-4 bg-gradient-to-r from-indigo-500 to-blue-500 hover:from-indigo-400 hover:to-blue-400 disabled:from-slate-600 disabled:to-slate-600 text-white font-semibold rounded-xl transition-all flex items-center justify-center gap-2"
                  >
                    {loading() ? (
                      <>
                        <svg class="w-5 h-5 animate-spin" fill="none" viewBox="0 0 24 24">
                          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                        </svg>
                        Retrieving...
                      </>
                    ) : (
                      <>
                        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
                        </svg>
                        Get My License Key
                      </>
                    )}
                  </button>
                </div>
              </Show>

              <Show when={licenseKey()}>
                <div class="space-y-5">
                  {/* License Key Card */}
                  <div class="bg-gradient-to-br from-indigo-500/10 to-purple-500/10 border border-indigo-500/30 rounded-2xl p-5">
                    <div class="flex items-center justify-between mb-3">
                      <div class="flex items-center gap-2">
                        <div class="px-2 py-1 bg-indigo-500/20 rounded-md">
                          <span class="text-xs font-bold text-indigo-400 uppercase">{tier()}</span>
                        </div>
                        <span class="text-sm text-slate-400">License Key</span>
                      </div>
                      <button
                        onClick={() => copyToClipboard(licenseKey()!)}
                        class="flex items-center gap-1.5 px-3 py-1.5 bg-slate-700/50 hover:bg-slate-600/50 rounded-lg text-sm text-slate-300 hover:text-white transition-all"
                      >
                        {copied() ? (
                          <>
                            <svg class="w-4 h-4 text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                            </svg>
                            Copied!
                          </>
                        ) : (
                          <>
                            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                            </svg>
                            Copy
                          </>
                        )}
                      </button>
                    </div>
                    <code class="block text-green-400 font-mono text-sm break-all bg-slate-900/50 p-3 rounded-lg">
                      {licenseKey()}
                    </code>
                  </div>

                  {/* Activation Instructions */}
                  <div class="bg-slate-800/50 border border-slate-700/50 rounded-2xl p-5">
                    <h3 class="text-white font-semibold mb-4 flex items-center gap-2">
                      <svg class="w-5 h-5 text-indigo-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
                      </svg>
                      Quick Activation
                    </h3>
                    
                    <div class="space-y-3">
                      <div class="flex items-start gap-3">
                        <div class="w-6 h-6 rounded-full bg-indigo-500/20 flex items-center justify-center flex-shrink-0 mt-0.5">
                          <span class="text-xs font-bold text-indigo-400">1</span>
                        </div>
                        <div class="flex-1">
                          <p class="text-sm text-slate-300">Open your terminal and run:</p>
                          <div class="mt-2 flex items-center gap-2">
                            <code class="flex-1 text-xs bg-slate-900 text-slate-300 px-3 py-2 rounded-lg font-mono">
                              omg license activate {licenseKey()}
                            </code>
                            <button
                              onClick={() => copyToClipboard(`omg license activate ${licenseKey()}`)}
                              class="p-2 bg-slate-700/50 hover:bg-slate-600/50 rounded-lg text-slate-400 hover:text-white transition-all"
                            >
                              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                              </svg>
                            </button>
                          </div>
                        </div>
                      </div>

                      <div class="flex items-start gap-3">
                        <div class="w-6 h-6 rounded-full bg-indigo-500/20 flex items-center justify-center flex-shrink-0 mt-0.5">
                          <span class="text-xs font-bold text-indigo-400">2</span>
                        </div>
                        <div>
                          <p class="text-sm text-slate-300">Verify with:</p>
                          <code class="text-xs bg-slate-900 text-slate-300 px-3 py-2 rounded-lg font-mono mt-2 inline-block">
                            omg license status
                          </code>
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* Features unlocked */}
                  <div class="bg-green-500/10 border border-green-500/30 rounded-xl p-4">
                    <p class="text-sm text-green-400 font-medium flex items-center gap-2">
                      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                      </svg>
                      You now have access to SBOM, vulnerability scanning, and secret detection!
                    </p>
                  </div>
                </div>
              </Show>

              <button
                onClick={handleClose}
                class="mt-6 w-full py-3 text-slate-400 hover:text-white hover:bg-slate-800/50 rounded-xl transition-all text-sm"
              >
                {licenseKey() ? "Done" : "Close"}
              </button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default App;
