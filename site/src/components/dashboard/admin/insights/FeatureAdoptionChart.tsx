import { Component, For } from 'solid-js';
import { Package, Search, Repeat, FileCode } from 'lucide-solid';

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

export const FeatureAdoptionChart: Component<FeatureAdoptionChartProps> = (props) => {
  const features = [
    {
      name: 'Package Install',
      icon: Package,
      adopters: props.data.install_adopters ?? 0,
      total_uses: props.data.total_installs ?? 0,
      color: 'indigo',
    },
    {
      name: 'Package Search',
      icon: Search,
      adopters: props.data.search_adopters ?? 0,
      total_uses: props.data.total_searches ?? 0,
      color: 'cyan',
    },
    {
      name: 'Runtime Switch',
      icon: Repeat,
      adopters: props.data.runtime_adopters ?? 0,
      total_uses: props.data.total_runtime_switches ?? 0,
      color: 'purple',
    },
    {
      name: 'SBOM Generate',
      icon: FileCode,
      adopters: props.data.sbom_adopters ?? 0,
      total_uses: props.data.total_sbom ?? 0,
      color: 'emerald',
    },
  ];

  const getAdoptionRate = (adopters: number): string => {
    const totalUsers = props.data.total_active_users ?? 0;
    if (totalUsers === 0) return '0';
    return ((adopters / totalUsers) * 100).toFixed(1);
  };

  const getColorClasses = (color: string) => {
    const colors: Record<string, { bg: string; text: string; bar: string }> = {
      indigo: {
        bg: 'bg-indigo-500/20',
        text: 'text-indigo-400',
        bar: 'bg-indigo-500',
      },
      cyan: {
        bg: 'bg-cyan-500/20',
        text: 'text-cyan-400',
        bar: 'bg-cyan-500',
      },
      purple: {
        bg: 'bg-purple-500/20',
        text: 'text-purple-400',
        bar: 'bg-purple-500',
      },
      emerald: {
        bg: 'bg-emerald-500/20',
        text: 'text-emerald-400',
        bar: 'bg-emerald-500',
      },
    };
    return colors[color] || colors.indigo;
  };

  return (
    <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
      <div class="mb-6">
        <h3 class="text-2xl font-black tracking-tight text-white">Feature Adoption</h3>
        <p class="mt-1 text-sm text-slate-500">
          Usage patterns across {(props.data.total_active_users ?? 0).toLocaleString()} active users
        </p>
      </div>

      <div class="space-y-4">
        <For each={features}>
          {(feature) => {
            const colors = getColorClasses(feature.color);
            const adoptionRate = parseFloat(getAdoptionRate(feature.adopters));
            const Icon = feature.icon;

            return (
              <div class="rounded-xl border border-white/10 bg-white/5 p-4">
                <div class="mb-3 flex items-center justify-between">
                  <div class="flex items-center gap-3">
                    <div class={`rounded-lg ${colors.bg} p-2`}>
                      <Icon size={18} class={colors.text} />
                    </div>
                    <div>
                      <p class="text-sm font-bold text-white">{feature.name}</p>
                      <p class="text-xs text-slate-400">
                        {feature.total_uses.toLocaleString()} total uses
                      </p>
                    </div>
                  </div>
                  <div class="text-right">
                    <p class={`text-2xl font-black ${colors.text}`}>{adoptionRate}%</p>
                    <p class="text-xs text-slate-500">{feature.adopters} users</p>
                  </div>
                </div>

                <div class="h-2 overflow-hidden rounded-full bg-white/5">
                  <div
                    class={`h-full rounded-full transition-all duration-500 ${colors.bar}`}
                    style={{ width: `${Math.min(adoptionRate, 100)}%` }}
                  />
                </div>
              </div>
            );
          }}
        </For>
      </div>

      <div class="mt-6 grid grid-cols-2 gap-4 rounded-xl border border-white/10 bg-white/5 p-4">
        <div>
          <p class="text-xs text-slate-400">Total Vulnerabilities Found</p>
          <p class="mt-1 text-xl font-bold text-rose-400">
            {(props.data.total_vulns ?? 0).toLocaleString()}
          </p>
        </div>
        <div>
          <p class="text-xs text-slate-400">SBOM Adopters</p>
          <p class="mt-1 text-xl font-bold text-emerald-400">
            {props.data.sbom_adopters ?? 0} users
          </p>
        </div>
      </div>
    </div>
  );
};
