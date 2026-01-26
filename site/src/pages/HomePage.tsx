import { Component, createSignal, onMount, Show, For, lazy, Suspense } from 'solid-js';
import Header from '../components/Header';
import Hero from '../components/Hero';
import FeatureGrid from '../components/landing/FeatureGrid';
import RuntimeEcosystem from '../components/RuntimeEcosystem';
import Benchmarks from '../components/Benchmarks';
import Pricing from '../components/Pricing';
import Installation from '../components/Installation';
import Footer from '../components/Footer';

const BackgroundMesh = lazy(() => import('../components/3d/BackgroundMesh'));

const CONFETTI_COLORS = ['#6366f1', '#8b5cf6', '#ec4899', '#10b981', '#f59e0b', '#3b82f6'];

const HomePage: Component = () => {
  const [showSuccess, setShowSuccess] = createSignal(false);
  const [show3D, setShow3D] = createSignal(false);

  onMount(() => {
    requestIdleCallback(() => setShow3D(true), { timeout: 2000 });
  });
  const [licenseKey, setLicenseKey] = createSignal<string | null>(null);
  const [tier, setTier] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [email, setEmail] = createSignal('');
  const [copied, setCopied] = createSignal(false);
  const [confetti, setConfetti] = createSignal<
    Array<{ id: number; left: number; color: string; delay: number }>
  >([]);
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
      const res = await fetch(
        `https://api.pyro1121.com/api/get-license?email=${encodeURIComponent(userEmail)}`
      );
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
      <Show when={show3D()}>
        <Suspense fallback={null}>
          <BackgroundMesh />
        </Suspense>
      </Show>
      <Header />
      <main>
        <Hero />
        <div class="relative z-10">
          <FeatureGrid />
          <RuntimeEcosystem />
          <Benchmarks />
          <Installation />
          <Pricing />
        </div>
      </main>
      <Footer />

      {/* Confetti */}
      <For each={confetti()}>
        {piece => (
          <div
            class="animate-confetti pointer-events-none fixed top-0 z-[200] h-3 w-3 rounded-full"
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
        <div class="fixed inset-0 z-[100] flex items-center justify-center p-4">
          <div class="absolute inset-0 bg-black/80 backdrop-blur-md" onClick={handleClose} />
          <div class="relative w-full max-w-lg rounded-3xl border border-slate-700/50 bg-gradient-to-b from-slate-800 to-slate-900 p-8 shadow-2xl">
            <button
              onClick={handleClose}
              class="absolute top-4 right-4 text-slate-400 hover:text-white"
            >
              <svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M6 18L18 6M6 6l12 12"
                />
              </svg>
            </button>

            <Show when={!licenseKey()}>
              <div class="text-center">
                <div class="mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-full bg-gradient-to-br from-green-400 to-emerald-500">
                  <svg
                    class="h-10 w-10 text-white"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M5 13l4 4L19 7"
                    />
                  </svg>
                </div>
                <h2 class="mb-2 text-3xl font-bold text-white">Payment Successful!</h2>
                <p class="mb-6 text-slate-400">
                  Thank you for your purchase. Enter your email to retrieve your license key.
                </p>

                <input
                  type="email"
                  value={email()}
                  onInput={e => setEmail(e.currentTarget.value)}
                  onKeyPress={e => e.key === 'Enter' && fetchLicense()}
                  placeholder="Enter your email"
                  class="mb-4 w-full rounded-xl border border-slate-600 bg-slate-800 px-4 py-3 text-white placeholder-slate-500 focus:border-indigo-500 focus:outline-none"
                />

                <Show when={notFound()}>
                  <p class="mb-4 text-sm text-amber-400">
                    License not found yet. It may take a moment to process.
                    {retryCount() > 0 && ` (Attempt ${retryCount()})`}
                  </p>
                </Show>

                <button
                  onClick={fetchLicense}
                  disabled={loading() || !email()}
                  class="w-full rounded-xl bg-gradient-to-r from-indigo-500 to-purple-500 py-3 font-semibold text-white transition-all hover:from-indigo-400 hover:to-purple-400 disabled:from-slate-600 disabled:to-slate-600"
                >
                  {loading() ? 'Checking...' : 'Get License Key'}
                </button>
              </div>
            </Show>

            <Show when={licenseKey()}>
              <div class="text-center">
                <div class="mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-full bg-gradient-to-br from-indigo-400 to-purple-500">
                  <svg
                    class="h-10 w-10 text-white"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z"
                    />
                  </svg>
                </div>
                <h2 class="mb-2 text-3xl font-bold text-white">Your License Key</h2>
                <p class="mb-2 text-slate-400">
                  <span class="font-semibold text-indigo-400 capitalize">{tier()}</span> Plan
                  Activated
                </p>

                <div class="mb-6 rounded-xl bg-slate-800 p-4">
                  <code class="font-mono text-sm break-all text-green-400">{licenseKey()}</code>
                </div>

                <button
                  onClick={() => copyToClipboard(licenseKey()!)}
                  class="mb-4 flex w-full items-center justify-center gap-2 rounded-xl bg-slate-700 py-3 font-semibold text-white transition-all hover:bg-slate-600"
                >
                  {copied() ? (
                    <>
                      <svg
                        class="h-5 w-5 text-green-400"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="2"
                          d="M5 13l4 4L19 7"
                        />
                      </svg>
                      Copied!
                    </>
                  ) : (
                    <>
                      <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="2"
                          d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                        />
                      </svg>
                      Copy to Clipboard
                    </>
                  )}
                </button>

                <div class="rounded-xl bg-slate-800/50 p-4 text-left">
                  <p class="mb-2 text-sm text-slate-300">Activate your license:</p>
                  <code class="font-mono text-xs text-cyan-400">
                    omg license activate {licenseKey()}
                  </code>
                </div>
              </div>
            </Show>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default HomePage;
