import { Component, For, Show, createMemo, createSignal, onMount } from 'solid-js';
import { Globe, MapPin, TrendingUp, ChevronDown, ChevronUp } from 'lucide-solid';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

interface GeoData {
  country_code: string;
  country_name?: string;
  user_count: number;
  percentage?: number;
}

interface GeoDistributionProps {
  data: GeoData[];
  maxItems?: number;
}

const COUNTRY_NAMES: Record<string, string> = {
  US: 'United States',
  GB: 'United Kingdom',
  DE: 'Germany',
  FR: 'France',
  CA: 'Canada',
  AU: 'Australia',
  JP: 'Japan',
  IN: 'India',
  BR: 'Brazil',
  NL: 'Netherlands',
  SE: 'Sweden',
  ES: 'Spain',
  IT: 'Italy',
  PL: 'Poland',
  RU: 'Russia',
  CN: 'China',
  KR: 'South Korea',
  MX: 'Mexico',
  AR: 'Argentina',
  ZA: 'South Africa',
  SG: 'Singapore',
  HK: 'Hong Kong',
  TW: 'Taiwan',
  NO: 'Norway',
  DK: 'Denmark',
  FI: 'Finland',
  CH: 'Switzerland',
  AT: 'Austria',
  BE: 'Belgium',
  IE: 'Ireland',
  NZ: 'New Zealand',
  PT: 'Portugal',
  CZ: 'Czech Republic',
  IL: 'Israel',
  UA: 'Ukraine',
  RO: 'Romania',
  TR: 'Turkey',
  TH: 'Thailand',
  VN: 'Vietnam',
  PH: 'Philippines',
  ID: 'Indonesia',
  MY: 'Malaysia',
  CL: 'Chile',
  CO: 'Colombia',
  PE: 'Peru',
};

const COUNTRY_FLAGS: Record<string, string> = {
  US: '🇺🇸', GB: '🇬🇧', DE: '🇩🇪', FR: '🇫🇷', CA: '🇨🇦',
  AU: '🇦🇺', JP: '🇯🇵', IN: '🇮🇳', BR: '🇧🇷', NL: '🇳🇱',
  SE: '🇸🇪', ES: '🇪🇸', IT: '🇮🇹', PL: '🇵🇱', RU: '🇷🇺',
  CN: '🇨🇳', KR: '🇰🇷', MX: '🇲🇽', AR: '🇦🇷', ZA: '🇿🇦',
  SG: '🇸🇬', HK: '🇭🇰', TW: '🇹🇼', NO: '🇳🇴', DK: '🇩🇰',
  FI: '🇫🇮', CH: '🇨🇭', AT: '🇦🇹', BE: '🇧🇪', IE: '🇮🇪',
  NZ: '🇳🇿', PT: '🇵🇹', CZ: '🇨🇿', IL: '🇮🇱', UA: '🇺🇦',
  RO: '🇷🇴', TR: '🇹🇷', TH: '🇹🇭', VN: '🇻🇳', PH: '🇵🇭',
  ID: '🇮🇩', MY: '🇲🇾', CL: '🇨🇱', CO: '🇨🇴', PE: '🇵🇪',
};

const REGION_COLORS = [
  { gradient: 'linear-gradient(135deg, var(--color-indigo-600), var(--color-indigo-400))', glow: 'rgba(99, 102, 241, 0.4)' },
  { gradient: 'linear-gradient(135deg, var(--color-electric-600), var(--color-electric-400))', glow: 'rgba(34, 211, 211, 0.4)' },
  { gradient: 'linear-gradient(135deg, var(--color-aurora-600), var(--color-aurora-400))', glow: 'rgba(16, 185, 129, 0.4)' },
  { gradient: 'linear-gradient(135deg, var(--color-photon-600), var(--color-photon-400))', glow: 'rgba(176, 109, 232, 0.4)' },
  { gradient: 'linear-gradient(135deg, var(--color-solar-600), var(--color-solar-400))', glow: 'rgba(245, 158, 11, 0.4)' },
];

export const GeoDistribution: Component<GeoDistributionProps> = (props) => {
  const [mounted, setMounted] = createSignal(false);
  const [showAll, setShowAll] = createSignal(false);
  const [hoveredCountry, setHoveredCountry] = createSignal<string | null>(null);

  onMount(() => {
    requestAnimationFrame(() => setMounted(true));
  });

  const maxItems = () => props.maxItems || 10;

  const sortedData = createMemo(() =>
    [...props.data].sort((a, b) => b.user_count - a.user_count)
  );

  const displayedData = createMemo(() =>
    showAll() ? sortedData() : sortedData().slice(0, maxItems())
  );

  const totalUsers = createMemo(() =>
    props.data.reduce((sum, item) => sum + item.user_count, 0)
  );

  const maxUsers = createMemo(() =>
    Math.max(...props.data.map((d) => d.user_count), 1)
  );

  const getCountryName = (code: string) =>
    COUNTRY_NAMES[code] || code;

  const getFlag = (code: string) =>
    COUNTRY_FLAGS[code] || '🌍';

  const getPercentage = (count: number) =>
    totalUsers() > 0 ? ((count / totalUsers()) * 100).toFixed(1) : '0';

  const getBarWidth = (count: number) =>
    (count / maxUsers()) * 100;

  const topRegions = createMemo(() => {
    const regions = {
      'North America': ['US', 'CA', 'MX'],
      'Europe': ['GB', 'DE', 'FR', 'NL', 'SE', 'ES', 'IT', 'PL', 'NO', 'DK', 'FI', 'CH', 'AT', 'BE', 'IE', 'PT', 'CZ', 'UA', 'RO'],
      'Asia Pacific': ['JP', 'IN', 'CN', 'KR', 'SG', 'HK', 'TW', 'TH', 'VN', 'PH', 'ID', 'MY', 'AU', 'NZ'],
      'Latin America': ['BR', 'AR', 'CL', 'CO', 'PE'],
      'Other': ['RU', 'ZA', 'IL', 'TR'],
    };

    const regionCounts: { name: string; count: number }[] = [];
    
    for (const [name, codes] of Object.entries(regions)) {
      const count = props.data
        .filter((d) => codes.includes(d.country_code))
        .reduce((sum, d) => sum + d.user_count, 0);
      if (count > 0) {
        regionCounts.push({ name, count });
      }
    }

    return regionCounts.sort((a, b) => b.count - a.count);
  });

  return (
    <div class="rounded-2xl border border-white/[0.06] bg-void-900 p-6 shadow-2xl relative overflow-hidden">
      <div
        class="absolute -top-20 -right-20 w-40 h-40 rounded-full blur-3xl opacity-15 transition-opacity duration-500"
        style={{ background: 'var(--color-indigo-500)' }}
      />

      <div class="mb-6 flex items-start justify-between relative">
        <div>
          <div class="flex items-center gap-3 mb-1">
            <div
              class="w-10 h-10 rounded-xl flex items-center justify-center"
              style={{
                background: 'linear-gradient(135deg, var(--color-indigo-600), var(--color-indigo-400))',
                'box-shadow': '0 0 15px rgba(99, 102, 241, 0.3)',
              }}
            >
              <Globe size={20} class="text-white" />
            </div>
            <div>
              <h3 class="text-lg font-bold tracking-tight text-nebula-100">Geographic Distribution</h3>
              <p class="text-xs text-nebula-500">
                <span class="font-bold text-nebula-300">{totalUsers().toLocaleString()}</span> users across{' '}
                <span class="font-bold text-nebula-300">{props.data.length}</span> countries
              </p>
            </div>
          </div>
        </div>
      </div>

      <Show when={props.data.length === 0}>
        <div class="flex flex-col items-center justify-center py-12">
          <div class="w-16 h-16 rounded-full bg-void-800 flex items-center justify-center mb-4">
            <Globe size={32} class="text-nebula-600" />
          </div>
          <p class="text-lg font-bold text-nebula-200">No Geographic Data</p>
          <p class="mt-1 text-sm text-nebula-500">Location data will appear once users connect</p>
        </div>
      </Show>

      <Show when={props.data.length > 0}>
        <div class="mb-6 grid grid-cols-2 md:grid-cols-4 gap-3">
          <For each={topRegions().slice(0, 4)}>
            {(region, index) => {
              const colors = REGION_COLORS[index() % REGION_COLORS.length];
              const percentage = totalUsers() > 0 ? ((region.count / totalUsers()) * 100).toFixed(0) : '0';
              
              return (
                <div
                  class={cn(
                    'rounded-xl border border-white/[0.06] bg-void-800/40 p-3',
                    'transition-all duration-300'
                  )}
                >
                  <p class="text-[10px] font-bold uppercase tracking-wider text-nebula-500 mb-1">
                    {region.name}
                  </p>
                  <div class="flex items-baseline gap-2">
                    <span class="text-xl font-black text-nebula-100">{percentage}%</span>
                    <span class="text-xs text-nebula-500">{region.count.toLocaleString()}</span>
                  </div>
                  <div class="mt-2 h-1 rounded-full bg-void-700 overflow-hidden">
                    <div
                      class="h-full rounded-full transition-all duration-700"
                      style={{
                        width: mounted() ? `${percentage}%` : '0%',
                        background: colors.gradient,
                      }}
                    />
                  </div>
                </div>
              );
            }}
          </For>
        </div>

        <div
          class={cn(
            'space-y-2 transition-all duration-500',
            mounted() ? 'opacity-100' : 'opacity-0'
          )}
        >
          <For each={displayedData()}>
            {(country, index) => {
              const isHovered = () => hoveredCountry() === country.country_code;
              const barWidth = getBarWidth(country.user_count);
              const isTopThree = index() < 3;
              const colors = REGION_COLORS[index() % REGION_COLORS.length];

              return (
                <div
                  class={cn(
                    'group relative rounded-xl border bg-void-800/30 p-3',
                    'transition-all duration-300 cursor-default',
                    isHovered() && 'bg-void-750/50 border-white/10',
                    !isHovered() && 'border-white/[0.04]'
                  )}
                  style={{
                    'animation-delay': `${index() * 30}ms`,
                  }}
                  onMouseEnter={() => setHoveredCountry(country.country_code)}
                  onMouseLeave={() => setHoveredCountry(null)}
                >
                  <div class="flex items-center gap-3">
                    <div
                      class={cn(
                        'w-8 h-8 rounded-lg flex items-center justify-center text-lg',
                        'transition-transform duration-300',
                        isHovered() && 'scale-110'
                      )}
                      style={{
                        background: isTopThree ? colors.gradient : 'var(--color-void-700)',
                        'box-shadow': isHovered() && isTopThree ? `0 0 12px ${colors.glow}` : undefined,
                      }}
                    >
                      {isTopThree ? (
                        <span class="text-sm">{getFlag(country.country_code)}</span>
                      ) : (
                        <span class="text-xs font-bold text-nebula-400">{index() + 1}</span>
                      )}
                    </div>

                    <div class="flex-1 min-w-0">
                      <div class="flex items-center gap-2">
                        <span class="text-sm font-semibold text-nebula-200 truncate">
                          {getCountryName(country.country_code)}
                        </span>
                        <Show when={!isTopThree}>
                          <span class="text-sm">{getFlag(country.country_code)}</span>
                        </Show>
                        <Show when={isTopThree}>
                          <span
                            class="text-[10px] px-1.5 py-0.5 rounded-full font-bold"
                            style={{
                              background: colors.glow,
                              color: 'white',
                            }}
                          >
                            Top {index() + 1}
                          </span>
                        </Show>
                      </div>
                      <div class="mt-1.5 h-1.5 rounded-full bg-void-700 overflow-hidden">
                        <div
                          class={cn(
                            'h-full rounded-full transition-all duration-700',
                            mounted() ? 'opacity-100' : 'opacity-0'
                          )}
                          style={{
                            width: mounted() ? `${barWidth}%` : '0%',
                            background: isTopThree ? colors.gradient : 'var(--color-nebula-600)',
                            'box-shadow': isHovered() ? `0 0 8px ${colors.glow}` : undefined,
                          }}
                        />
                      </div>
                    </div>

                    <div class="text-right shrink-0">
                      <p class="text-sm font-bold tabular-nums text-nebula-100">
                        {country.user_count.toLocaleString()}
                      </p>
                      <p class="text-[10px] text-nebula-500 tabular-nums">
                        {getPercentage(country.user_count)}%
                      </p>
                    </div>
                  </div>
                </div>
              );
            }}
          </For>
        </div>

        <Show when={sortedData().length > maxItems()}>
          <button
            onClick={() => setShowAll(!showAll())}
            class={cn(
              'mt-4 w-full py-2.5 text-sm font-medium rounded-xl',
              'border border-white/[0.06] bg-void-800/30',
              'text-nebula-400 hover:text-nebula-200',
              'transition-all duration-200 hover:bg-void-750/50',
              'flex items-center justify-center gap-2'
            )}
          >
            {showAll() ? (
              <>
                <ChevronUp size={16} />
                Show Less
              </>
            ) : (
              <>
                <ChevronDown size={16} />
                Show All {sortedData().length} Countries
              </>
            )}
          </button>
        </Show>

        <div class="mt-6 rounded-xl border border-white/[0.06] bg-void-800/30 p-4">
          <div class="flex items-center gap-3">
            <div
              class="w-8 h-8 rounded-lg flex items-center justify-center"
              style={{ background: 'rgba(99, 102, 241, 0.15)' }}
            >
              <TrendingUp size={14} class="text-indigo-400" />
            </div>
            <div>
              <p class="text-sm font-semibold text-nebula-100">Geographic Insight</p>
              <p class="mt-0.5 text-xs text-nebula-400">
                {sortedData()[0]
                  ? `${getCountryName(sortedData()[0].country_code)} leads with ${getPercentage(sortedData()[0].user_count)}% of users. Consider localization for top markets.`
                  : 'Geographic distribution data will help identify key markets.'}
              </p>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default GeoDistribution;
