import { Component, createSignal, Show, For } from 'solid-js';

interface UpgradeModalProps {
  isOpen: boolean;
  onClose: () => void;
  initialTier?: 'pro' | 'team';
}

const TIERS = {
  pro: {
    name: 'Pro',
    price: 9,
    priceId: 'price_1SqWTHEFWzXv59QZC5zBGdLg',
    description: 'For security-conscious developers',
    color: 'indigo',
    features: [
      { name: 'SBOM Generation', description: 'Software Bill of Materials for compliance' },
      { name: 'Vulnerability Scanning', description: 'Detect CVEs in your dependencies' },
      { name: 'Secret Detection', description: 'Find leaked credentials before they ship' },
      { name: 'Priority Updates', description: 'Get new features first' },
    ],
  },
  team: {
    name: 'Team',
    price: 200,
    priceId: 'price_1SqWTXEFWzXv59QZB5G51MOV',
    description: 'For teams & organizations',
    color: 'purple',
    features: [
      { name: 'Everything in Pro', description: 'All security features included' },
      { name: 'Team Environment Sync', description: 'Keep everyone on the same page' },
      { name: 'Shared Configurations', description: 'Centralized team settings' },
      { name: 'Audit Logs', description: 'Tamper-proof activity tracking' },
      { name: 'Up to 25 Members', description: 'Scale your team securely' },
    ],
  },
};

const UpgradeModal: Component<UpgradeModalProps> = (props) => {
  const [step, setStep] = createSignal<'select' | 'details' | 'processing'>('select');
  const [selectedTier, setSelectedTier] = createSignal<'pro' | 'team'>(props.initialTier || 'pro');
  const [email, setEmail] = createSignal('');
  const [name, setName] = createSignal('');
  const [error, setError] = createSignal<string | null>(null);
  const [isLoading, setIsLoading] = createSignal(false);

  const handleTierSelect = (tier: 'pro' | 'team') => {
    setSelectedTier(tier);
    setStep('details');
  };

  const handleBack = () => {
    if (step() === 'details') setStep('select');
  };

  const handleCheckout = async () => {
    if (!email() || !email().includes('@')) {
      setError('Please enter a valid email address');
      return;
    }

    setError(null);
    setIsLoading(true);
    setStep('processing');

    try {
      const tier = TIERS[selectedTier()];
      const res = await fetch('https://api.pyro1121.com/api/create-checkout', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ 
          email: email(), 
          priceId: tier.priceId,
          name: name() || undefined,
        }),
      });

      const data = await res.json();
      
      if (data.url) {
        // Small delay for animation
        setTimeout(() => {
          window.location.href = data.url;
        }, 800);
      } else {
        setError(data.error || 'Failed to create checkout session');
        setStep('details');
      }
    } catch (e) {
      setError('Network error. Please try again.');
      setStep('details');
    } finally {
      setIsLoading(false);
    }
  };

  const handleClose = () => {
    setStep('select');
    setEmail('');
    setName('');
    setError(null);
    props.onClose();
  };

  return (
    <Show when={props.isOpen}>
      <div 
        class="fixed inset-0 z-50 flex items-center justify-center p-4"
        onClick={(e) => e.target === e.currentTarget && handleClose()}
      >
        {/* Backdrop */}
        <div class="absolute inset-0 bg-black/80 backdrop-blur-md animate-fade-in" />
        
        {/* Modal */}
        <div class="relative bg-gradient-to-b from-slate-800 to-slate-900 border border-slate-700/50 rounded-3xl max-w-4xl w-full shadow-2xl shadow-black/50 animate-scale-in overflow-hidden">
          {/* Glow effect */}
          <div class="absolute -top-40 -right-40 w-80 h-80 bg-indigo-500/20 rounded-full blur-3xl pointer-events-none" />
          <div class="absolute -bottom-40 -left-40 w-80 h-80 bg-purple-500/20 rounded-full blur-3xl pointer-events-none" />
          
          {/* Close button */}
          <button
            onClick={handleClose}
            class="absolute top-4 right-4 z-10 p-2 text-slate-400 hover:text-white hover:bg-slate-700/50 rounded-full transition-all"
          >
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>

          <div class="relative p-8">
            {/* Step 1: Tier Selection */}
            <Show when={step() === 'select'}>
              <div class="text-center mb-8">
                <div class="inline-flex items-center gap-2 px-4 py-2 bg-indigo-500/10 border border-indigo-500/30 rounded-full text-indigo-400 text-sm font-medium mb-4">
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
                  </svg>
                  Upgrade Your Experience
                </div>
                <h2 class="text-3xl font-bold text-white mb-2">Choose Your Plan</h2>
                <p class="text-slate-400">Unlock powerful features for your development workflow</p>
              </div>

              <div class="grid md:grid-cols-2 gap-6">
                <For each={(['pro', 'team'] as const)}>
                  {(tierKey) => {
                    const tier = TIERS[tierKey];
                    const isTeam = tierKey === 'team';
                    return (
                      <button
                        onClick={() => handleTierSelect(tierKey)}
                        class={`relative group text-left p-6 rounded-2xl border-2 transition-all duration-300 hover:scale-[1.02] ${
                          isTeam 
                            ? 'border-purple-500/50 bg-purple-500/5 hover:border-purple-400 hover:bg-purple-500/10' 
                            : 'border-slate-600/50 bg-slate-800/50 hover:border-indigo-400 hover:bg-indigo-500/10'
                        }`}
                      >
                        {isTeam && (
                          <div class="absolute -top-3 left-6">
                            <span class="px-3 py-1 bg-gradient-to-r from-purple-500 to-pink-500 text-white text-xs font-bold rounded-full shadow-lg">
                              BEST VALUE
                            </span>
                          </div>
                        )}
                        
                        <div class="flex items-start justify-between mb-4">
                          <div>
                            <h3 class="text-xl font-bold text-white mb-1">{tier.name}</h3>
                            <p class="text-sm text-slate-400">{tier.description}</p>
                          </div>
                          <div class={`p-3 rounded-xl ${isTeam ? 'bg-purple-500/20' : 'bg-indigo-500/20'}`}>
                            <svg class={`w-6 h-6 ${isTeam ? 'text-purple-400' : 'text-indigo-400'}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                              {isTeam ? (
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
                              ) : (
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                              )}
                            </svg>
                          </div>
                        </div>

                        <div class="flex items-baseline gap-1 mb-6">
                          <span class="text-4xl font-bold text-white">${tier.price}</span>
                          <span class="text-slate-400">/month</span>
                        </div>

                        <ul class="space-y-3 mb-6">
                          <For each={tier.features.slice(0, 4)}>
                            {(feature) => (
                              <li class="flex items-start gap-3">
                                <svg class={`w-5 h-5 mt-0.5 flex-shrink-0 ${isTeam ? 'text-purple-400' : 'text-green-400'}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                                </svg>
                                <div>
                                  <span class="text-white font-medium">{feature.name}</span>
                                  <p class="text-xs text-slate-500">{feature.description}</p>
                                </div>
                              </li>
                            )}
                          </For>
                        </ul>

                        <div class={`flex items-center justify-center gap-2 py-3 rounded-xl font-semibold transition-all ${
                          isTeam 
                            ? 'bg-purple-500 text-white group-hover:bg-purple-400' 
                            : 'bg-indigo-500 text-white group-hover:bg-indigo-400'
                        }`}>
                          Select {tier.name}
                          <svg class="w-4 h-4 group-hover:translate-x-1 transition-transform" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                          </svg>
                        </div>
                      </button>
                    );
                  }}
                </For>
              </div>

              <p class="text-center text-slate-500 text-sm mt-6">
                All plans include a 14-day money-back guarantee
              </p>
            </Show>

            {/* Step 2: Email & Details */}
            <Show when={step() === 'details'}>
              <button
                onClick={handleBack}
                class="flex items-center gap-2 text-slate-400 hover:text-white transition-colors mb-6"
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
                </svg>
                Back to plans
              </button>

              <div class="grid md:grid-cols-2 gap-8">
                {/* Left: Form */}
                <div>
                  <div class="mb-6">
                    <div class={`inline-flex items-center gap-2 px-3 py-1.5 rounded-full text-sm font-medium ${
                      selectedTier() === 'team' 
                        ? 'bg-purple-500/20 text-purple-400' 
                        : 'bg-indigo-500/20 text-indigo-400'
                    }`}>
                      {TIERS[selectedTier()].name} Plan
                    </div>
                    <h2 class="text-2xl font-bold text-white mt-3 mb-1">Complete Your Upgrade</h2>
                    <p class="text-slate-400 text-sm">Enter your details to continue to secure checkout</p>
                  </div>

                  <div class="space-y-4">
                    <div>
                      <label class="block text-sm font-medium text-slate-300 mb-2">
                        Email Address <span class="text-red-400">*</span>
                      </label>
                      <input
                        type="email"
                        value={email()}
                        onInput={(e) => setEmail(e.currentTarget.value)}
                        placeholder="you@company.com"
                        class="w-full px-4 py-3 bg-slate-900/50 border border-slate-600 rounded-xl text-white placeholder-slate-500 focus:outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 transition-all"
                      />
                    </div>

                    <div>
                      <label class="block text-sm font-medium text-slate-300 mb-2">
                        Full Name <span class="text-slate-500">(optional)</span>
                      </label>
                      <input
                        type="text"
                        value={name()}
                        onInput={(e) => setName(e.currentTarget.value)}
                        placeholder="John Doe"
                        class="w-full px-4 py-3 bg-slate-900/50 border border-slate-600 rounded-xl text-white placeholder-slate-500 focus:outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-500/20 transition-all"
                      />
                    </div>

                    <Show when={error()}>
                      <div class="flex items-center gap-2 p-3 bg-red-500/10 border border-red-500/30 rounded-xl text-red-400 text-sm">
                        <svg class="w-4 h-4 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                        </svg>
                        {error()}
                      </div>
                    </Show>

                    <button
                      onClick={handleCheckout}
                      disabled={isLoading()}
                      class={`w-full py-4 rounded-xl font-semibold text-white transition-all flex items-center justify-center gap-2 ${
                        selectedTier() === 'team'
                          ? 'bg-gradient-to-r from-purple-500 to-pink-500 hover:from-purple-400 hover:to-pink-400'
                          : 'bg-gradient-to-r from-indigo-500 to-blue-500 hover:from-indigo-400 hover:to-blue-400'
                      } disabled:opacity-50 disabled:cursor-not-allowed`}
                    >
                      {isLoading() ? (
                        <>
                          <svg class="w-5 h-5 animate-spin" fill="none" viewBox="0 0 24 24">
                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                          </svg>
                          Processing...
                        </>
                      ) : (
                        <>
                          Continue to Checkout
                          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
                          </svg>
                        </>
                      )}
                    </button>

                    <div class="flex items-center justify-center gap-4 text-xs text-slate-500">
                      <div class="flex items-center gap-1">
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
                        </svg>
                        Secure checkout
                      </div>
                      <div class="flex items-center gap-1">
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 10h18M7 15h1m4 0h1m-7 4h12a3 3 0 003-3V8a3 3 0 00-3-3H6a3 3 0 00-3 3v8a3 3 0 003 3z" />
                        </svg>
                        Powered by Stripe
                      </div>
                    </div>
                  </div>
                </div>

                {/* Right: Order Summary */}
                <div class="bg-slate-800/50 rounded-2xl p-6 border border-slate-700/50">
                  <h3 class="text-lg font-semibold text-white mb-4">Order Summary</h3>
                  
                  <div class="flex items-center justify-between py-4 border-b border-slate-700">
                    <div>
                      <p class="text-white font-medium">OMG {TIERS[selectedTier()].name}</p>
                      <p class="text-sm text-slate-400">Monthly subscription</p>
                    </div>
                    <p class="text-xl font-bold text-white">${TIERS[selectedTier()].price}</p>
                  </div>

                  <div class="py-4 border-b border-slate-700">
                    <p class="text-sm text-slate-400 mb-3">What's included:</p>
                    <ul class="space-y-2">
                      <For each={TIERS[selectedTier()].features}>
                        {(feature) => (
                          <li class="flex items-center gap-2 text-sm text-slate-300">
                            <svg class="w-4 h-4 text-green-400 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                            </svg>
                            {feature.name}
                          </li>
                        )}
                      </For>
                    </ul>
                  </div>

                  <div class="flex items-center justify-between pt-4">
                    <p class="text-slate-400">Total today</p>
                    <p class="text-2xl font-bold text-white">${TIERS[selectedTier()].price}<span class="text-sm text-slate-400 font-normal">/mo</span></p>
                  </div>

                  <div class="mt-4 p-3 bg-green-500/10 border border-green-500/30 rounded-xl">
                    <p class="text-sm text-green-400 flex items-center gap-2">
                      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                      </svg>
                      14-day money-back guarantee
                    </p>
                  </div>
                </div>
              </div>
            </Show>

            {/* Step 3: Processing */}
            <Show when={step() === 'processing'}>
              <div class="text-center py-12">
                <div class="relative w-20 h-20 mx-auto mb-6">
                  <div class="absolute inset-0 rounded-full border-4 border-slate-700" />
                  <div class="absolute inset-0 rounded-full border-4 border-indigo-500 border-t-transparent animate-spin" />
                </div>
                <h2 class="text-2xl font-bold text-white mb-2">Preparing Checkout</h2>
                <p class="text-slate-400">Redirecting you to secure payment...</p>
              </div>
            </Show>
          </div>
        </div>
      </div>
    </Show>
  );
};

export default UpgradeModal;
