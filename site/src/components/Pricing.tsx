import { Component, createSignal } from 'solid-js';
import UpgradeModal from './UpgradeModal';

const Pricing: Component = () => {
  const [showUpgradeModal, setShowUpgradeModal] = createSignal(false);
  const [initialTier, setInitialTier] = createSignal<'pro' | 'team'>('pro');

  const openUpgrade = (tier: 'pro' | 'team') => {
    setInitialTier(tier);
    setShowUpgradeModal(true);
  };

  return (
    <section id="pricing" class="py-24 px-6">
      <div class="max-w-7xl mx-auto">
        <div class="text-center mb-16">
          <h2 class="text-4xl md:text-5xl font-bold mb-4">
            Simple, Transparent Pricing
          </h2>
          <p class="text-xl text-slate-400 max-w-2xl mx-auto">
            Start free, upgrade when you need security scanning, team features, or enterprise controls.
          </p>
        </div>

        <div class="grid md:grid-cols-2 lg:grid-cols-4 gap-6 max-w-7xl mx-auto">
          {/* Free Tier */}
          <div class="pricing-card">
            <div class="mb-6">
              <h3 class="text-2xl font-bold mb-2">Free</h3>
              <p class="text-slate-400 text-sm">For individual developers</p>
            </div>
            
            <div class="mb-6">
              <span class="text-4xl font-bold">$0</span>
              <span class="text-slate-400 text-sm">/forever</span>
            </div>

            <ul class="space-y-3 mb-6 text-sm">
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                Package management
              </li>
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                100+ runtimes via mise
              </li>
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                Container integration
              </li>
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                Environment fingerprinting
              </li>
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                Gist sharing
              </li>
            </ul>

            <a href="#install" class="btn-secondary w-full justify-center text-sm">
              Get Started
            </a>
          </div>

          {/* Pro Tier */}
          <div class="pricing-card featured">
            <div class="absolute -top-3 left-1/2 -translate-x-1/2">
              <span class="bg-indigo-500 text-white text-xs font-semibold px-3 py-1 rounded-full">
                Popular
              </span>
            </div>
            
            <div class="mb-6">
              <h3 class="text-2xl font-bold mb-2">Pro</h3>
              <p class="text-slate-400 text-sm">For security-conscious devs</p>
            </div>
            
            <div class="mb-6">
              <span class="text-4xl font-bold">$9</span>
              <span class="text-slate-400 text-sm">/month</span>
            </div>

            <ul class="space-y-3 mb-6 text-sm">
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                Everything in Free
              </li>
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                <strong>SBOM generation</strong>
              </li>
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                <strong>Vulnerability scanning</strong>
              </li>
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                <strong>Secret detection</strong>
              </li>
            </ul>

            <button 
              class="btn-primary w-full justify-center text-sm"
              onClick={() => openUpgrade('pro')}
            >
              Upgrade to Pro
            </button>
          </div>

          {/* Team Tier */}
          <div class="pricing-card featured">
            <div class="absolute -top-3 left-1/2 -translate-x-1/2">
              <span class="bg-purple-500 text-white text-xs font-semibold px-3 py-1 rounded-full">
                Best Value
              </span>
            </div>
            
            <div class="mb-6">
              <h3 class="text-2xl font-bold mb-2">Team</h3>
              <p class="text-slate-400 text-sm">For teams & organizations</p>
            </div>
            
            <div class="mb-6">
              <span class="text-4xl font-bold">$200</span>
              <span class="text-slate-400 text-sm">/month</span>
            </div>

            <ul class="space-y-3 mb-6 text-sm">
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                Everything in Pro
              </li>
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                <strong>Team environment sync</strong>
              </li>
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                <strong>Shared team configs</strong>
              </li>
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                <strong>Tamper-proof audit logs</strong>
              </li>
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                <strong>Up to 25 team members</strong>
              </li>
            </ul>

            <button 
              class="btn-primary w-full justify-center text-sm"
              onClick={() => openUpgrade('team')}
            >
              Upgrade to Team
            </button>
          </div>

          {/* Enterprise Tier */}
          <div class="pricing-card">
            <div class="mb-6">
              <h3 class="text-2xl font-bold mb-2">Enterprise</h3>
              <p class="text-slate-400 text-sm">For large organizations</p>
            </div>
            
            <div class="mb-6">
              <span class="text-3xl font-bold">Custom</span>
              <span class="text-slate-400 text-sm block mt-1">tailored to your needs</span>
            </div>

            <ul class="space-y-3 mb-6 text-sm">
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                Everything in Team
              </li>
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                <strong>SSO/SAML integration</strong>
              </li>
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                <strong>Policy enforcement</strong>
              </li>
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                <strong>SLSA provenance</strong>
              </li>
              <li class="flex items-center gap-2">
                <span class="text-green-400">✓</span>
                <strong>Dedicated support</strong>
              </li>
            </ul>

            <a 
              href="mailto:enterprise@pyro1121.com"
              class="btn-secondary w-full justify-center text-sm"
            >
              Contact Sales
            </a>
          </div>
        </div>

        <p class="text-center text-slate-500 mt-8">
          Need custom terms or volume pricing? <a href="mailto:sales@pyro1121.com" class="text-indigo-400 hover:underline">Contact sales</a>
        </p>
      </div>

      <UpgradeModal 
        isOpen={showUpgradeModal()} 
        onClose={() => setShowUpgradeModal(false)}
        initialTier={initialTier()}
      />
    </section>
  );
};

export default Pricing;
