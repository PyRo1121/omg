import { Component, For, createSignal, createMemo, onMount } from 'solid-js';
import { Package, Search, Repeat, FileCode, Shield, TrendingUp, Sparkles } from 'lucide-solid';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface FeatureAdoptionData {
  total_installs: number;
  total_searches: number;
  total_runtime_switches: number;
  total_sbom: number;
  total_vulns: number;
  install_adopters: number;
  search_adopters: number;
  runtime_adopters: number;
  sbom_adopters: number;
  total_active_users: number;
}

interface FeatureAdoptionChartProps {
  data: FeatureAdoptionData;
}

const featureConfig = {
  install: {
    name: 'Package Install',
    description: 'Users who installed packages',
    icon: Package,
    gradient: 'linear-gradient(135deg, var(--color-indigo-600), var(--color-indigo-400))',
    glow: 'var(--color-indigo-500)',
    bgClass: 'bg-indigo-500/15',
    textClass: 'text-indigo-400',
  },
  search: {
    name: 'Package Search',
    description: 'Users who searched packages',
    icon: Search,
    gradient: 'linear-gradient(135deg, var(--color-electric-600), var(--color-electric-400))',
    glow: 'var(--color-electric-500)',
    bgClass: 'bg-electric-500/15',
    textClass: 'text-electric-400',
  },
  runtime: {
    name: 'Runtime Switch',
    description: 'Users who switched runtimes',
    icon: Repeat,
    gradient: 'linear-gradient(135deg, var(--color-photon-600), var(--color-photon-400))',
    glow: 'var(--color-photon-500)',
    bgClass: 'bg-photon-500/15',
    textClass: 'text-photon-400',
  },
  sbom: {
    name: 'SBOM Generate',
    description: 'Security-conscious users',
    icon: FileCode,
    gradient: 'linear-gradient(135deg, var(--color-aurora-600), var(--color-aurora-400))',
    glow: 'var(--color-aurora-500)',
    bgClass: 'bg-aurora-500/15',
    textClass: 'text-aurora-400',
  },
} as const;

type FeatureKey = keyof typeof featureConfig;

export const FeatureAdoptionChart: Component<FeatureAdoptionChartProps> = (props) => {
  const [mounted, setMounted] = createSignal(false);
  const [hoveredFeature, setHoveredFeature] = createSignal<FeatureKey | null>(null);

  onMount(() => {
    requestAnimationFrame(() => setMounted(true));
  });

  const features = createMemo(() => [
    {
      key: 'install' as FeatureKey,
      adopters: props.data.install_adopters ?? 0,
      total_uses: props.data.total_installs ?? 0,
    },
    {
      key: 'search' as FeatureKey,
      adopters: props.data.search_adopters ?? 0,
      total_uses: props.data.total_searches ?? 0,
    },
    {
      key: 'runtime' as FeatureKey,
      adopters: props.data.runtime_adopters ?? 0,
      total_uses: props.data.total_runtime_switches ?? 0,
    },
    {
      key: 'sbom' as FeatureKey,
      adopters: props.data.sbom_adopters ?? 0,
      total_uses: props.data.total_sbom ?? 0,
    },
  ]);

  const totalActiveUsers = createMemo(() => props.data.total_active_users ?? 0);

  const getAdoptionRate = (adopters: number): number => {
    if (totalActiveUsers() === 0) return 0;
    return (adopters / totalActiveUsers()) * 100;
  };

  const overallAdoption = createMemo(() => {
    const rates = features().map(f => getAdoptionRate(f.adopters));
    return rates.reduce((sum, r) => sum + r, 0) / rates.length;
  });

  const adoptionHealth = createMemo(() => {
    const score = overallAdoption();
    if (score >= 60) return { label: 'Excellent', color: 'var(--health-excellent)', glow: 'var(--health-excellent-glow)' };
    if (score >= 40) return { label: 'Good', color: 'var(--health-good)', glow: 'var(--health-good-glow)' };
    if (score >= 25) return { label: 'Fair', color: 'var(--health-fair)', glow: 'var(--health-fair-glow)' };
    return { label: 'Growing', color: 'var(--color-plasma-400)', glow: 'rgba(90, 154, 232, 0.3)' };
  });

  return (
    <div class="rounded-2xl border border-white/[0.06] bg-void-900 p-6 shadow-2xl relative overflow-hidden">
      <div
        class="absolute -top-20 -right-20 w-40 h-40 rounded-full blur-3xl opacity-20 transition-opacity duration-500"
        style={{
          background: hoveredFeature() 
            ? featureConfig[hoveredFeature()!].glow 
            : 'var(--color-indigo-500)',
        }}
      />

      <div class="mb-6 flex items-start justify-between">
        <div>
          <div class="flex items-center gap-2 mb-1">
            <Sparkles size={20} class="text-indigo-400" />
            <h3 class="text-lg font-bold tracking-tight text-nebula-100">Feature Adoption</h3>
          </div>
          <p class="text-xs text-nebula-500">
            Usage patterns across{' '}
            <span class="text-nebula-300 font-medium tabular-nums">
              {totalActiveUsers().toLocaleString()}
            </span>{' '}
            active users
          </p>
        </div>
        
        <div
          class={cn(
            'px-3 py-1.5 rounded-full text-xs font-bold tabular-nums',
            'border transition-all duration-300'
          )}
          style={{
            color: adoptionHealth().color,
            'background-color': `color-mix(in srgb, ${adoptionHealth().color} 10%, transparent)`,
            'border-color': `color-mix(in srgb, ${adoptionHealth().color} 20%, transparent)`,
            'box-shadow': `0 0 12px ${adoptionHealth().glow}`,
          }}
        >
          {overallAdoption().toFixed(0)}% avg
        </div>
      </div>

      <div class="space-y-3">
        <For each={features()}>
          {(feature, index) => {
            const config = featureConfig[feature.key];
            const adoptionRate = getAdoptionRate(feature.adopters);
            const Icon = config.icon;
            const isHovered = () => hoveredFeature() === feature.key;

            return (
              <div
                class={cn(
                  'relative rounded-xl border bg-void-800/50 p-4',
                  'transition-all duration-300 cursor-default',
                  'hover:bg-void-750/70 hover:border-white/10',
                  isHovered() && 'border-white/15'
                )}
                style={{
                  'border-color': isHovered() ? `color-mix(in srgb, ${config.glow} 30%, transparent)` : undefined,
                  'box-shadow': isHovered() ? `0 0 20px ${config.glow}40, inset 0 1px 0 rgba(255,255,255,0.05)` : undefined,
                  'animation-delay': `${index() * 100}ms`,
                }}
                onMouseEnter={() => setHoveredFeature(feature.key)}
                onMouseLeave={() => setHoveredFeature(null)}
              >
                <div class="flex items-center justify-between mb-3">
                  <div class="flex items-center gap-3">
                    <div
                      class={cn(
                        'w-10 h-10 rounded-lg flex items-center justify-center',
                        'transition-transform duration-300',
                        isHovered() && 'scale-110'
                      )}
                      style={{
                        background: config.gradient,
                        'box-shadow': isHovered() ? `0 0 16px ${config.glow}60` : `0 0 8px ${config.glow}30`,
                      }}
                    >
                      <Icon size={20} class="text-white" />
                    </div>
                    <div>
                      <p class="text-sm font-semibold text-nebula-200">{config.name}</p>
                      <p class="text-xs text-nebula-500">
                        {feature.total_uses.toLocaleString()} total uses
                      </p>
                    </div>
                  </div>
                  
                  <div class="text-right">
                    <p
                      class={cn(
                        'text-2xl font-black tabular-nums transition-all duration-300',
                        mounted() ? 'opacity-100' : 'opacity-0'
                      )}
                      style={{ color: config.glow }}
                    >
                      {adoptionRate.toFixed(1)}%
                    </p>
                    <p class="text-xs text-nebula-500">
                      {feature.adopters.toLocaleString()} users
                    </p>
                  </div>
                </div>

                <div class="h-2 rounded-full bg-void-700 overflow-hidden">
                  <div
                    class={cn(
                      'h-full rounded-full transition-all duration-1000 ease-out',
                      mounted() ? 'opacity-100' : 'opacity-0 w-0'
                    )}
                    style={{
                      width: mounted() ? `${Math.min(adoptionRate, 100)}%` : '0%',
                      background: config.gradient,
                      'box-shadow': isHovered() ? `0 0 10px ${config.glow}` : undefined,
                    }}
                  />
                </div>
              </div>
            );
          }}
        </For>
      </div>

      <div class="mt-6 grid grid-cols-2 gap-3">
        <div
          class={cn(
            'rounded-xl border border-white/[0.06] bg-void-800/30 p-4',
            'transition-all duration-300 hover:bg-void-750/50 hover:border-flare-500/20'
          )}
          style={{
            'box-shadow': (props.data.total_vulns ?? 0) > 10 
              ? '0 0 15px var(--health-poor-glow)' 
              : undefined,
          }}
        >
          <div class="flex items-center gap-2 mb-2">
            <Shield size={14} class="text-flare-400" />
            <span class="text-2xs font-bold uppercase tracking-wider text-nebula-500">
              Vulnerabilities
            </span>
          </div>
          <p
            class="text-xl font-black tabular-nums"
            style={{
              color: (props.data.total_vulns ?? 0) > 10 
                ? 'var(--color-flare-400)' 
                : (props.data.total_vulns ?? 0) > 0 
                  ? 'var(--color-solar-400)' 
                  : 'var(--color-aurora-400)',
            }}
          >
            {(props.data.total_vulns ?? 0).toLocaleString()}
          </p>
          <p class="text-2xs text-nebula-600 mt-1">found across codebase</p>
        </div>

        <div
          class={cn(
            'rounded-xl border border-white/[0.06] bg-void-800/30 p-4',
            'transition-all duration-300 hover:bg-void-750/50 hover:border-aurora-500/20'
          )}
        >
          <div class="flex items-center gap-2 mb-2">
            <TrendingUp size={14} class="text-aurora-400" />
            <span class="text-2xs font-bold uppercase tracking-wider text-nebula-500">
              Security Adopters
            </span>
          </div>
          <p class="text-xl font-black text-aurora-400 tabular-nums">
            {props.data.sbom_adopters ?? 0}
          </p>
          <p class="text-2xs text-nebula-600 mt-1">
            {totalActiveUsers() > 0 
              ? `${((props.data.sbom_adopters ?? 0) / totalActiveUsers() * 100).toFixed(0)}% of users`
              : 'generating SBOMs'
            }
          </p>
        </div>
      </div>
    </div>
  );
};
