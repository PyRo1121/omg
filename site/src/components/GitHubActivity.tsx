import { Component, createSignal, onMount, Show } from 'solid-js';

interface WeekData {
  total: number;
  week: number;
}

interface CachedData {
  data: Array<{ label: string; value: number }>;
  total: number;
  timestamp: number;
}

const CACHE_KEY = 'github-activity-cache';
const CACHE_TTL = 2 * 60 * 1000;

const GitHubActivity: Component = () => {
  const [data, setData] = createSignal<Array<{ label: string; value: number }>>([]);
  const [loading, setLoading] = createSignal(true);
  const [totalCommits, setTotalCommits] = createSignal(0);
  const [error, setError] = createSignal<string | null>(null);

  const formatWeekLabel = (timestamp: number): string => {
    const date = new Date(timestamp * 1000);
    return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
  };

  const getCachedData = (): CachedData | null => {
    try {
      const cached = localStorage.getItem(CACHE_KEY);
      if (!cached) return null;
      const parsed: CachedData = JSON.parse(cached);
      if (Date.now() - parsed.timestamp > CACHE_TTL) return null;
      return parsed;
    } catch {
      return null;
    }
  };

  const setCachedData = (data: Array<{ label: string; value: number }>, total: number) => {
    try {
      const cache: CachedData = { data, total, timestamp: Date.now() };
      localStorage.setItem(CACHE_KEY, JSON.stringify(cache));
    } catch {
    }
  };

  onMount(async () => {
    const cached = getCachedData();
    if (cached) {
      setData(cached.data);
      setTotalCommits(cached.total);
      setLoading(false);
      return;
    }

    try {
      let response = await fetch('https://api.pyro1121.com/api/github-stats');

      if (!response.ok) {
        console.warn('Proxy failed, falling back to direct GitHub API');
        response = await fetch(
          'https://api.github.com/repos/PyRo1121/omg/stats/commit_activity',
          { headers: { Accept: 'application/vnd.github.v3+json' } }
        );
      }

      if (response.status === 202) {
        setError('GitHub is computing statistics. Refresh in 60 seconds.');
        setLoading(false);
        return;
      }

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const responseData = await response.json();

      if (responseData.computing) {
        setError(responseData.message || 'GitHub is computing statistics');
        setLoading(false);
        return;
      }

      const weeks: WeekData[] = responseData;

      if (!Array.isArray(weeks) || weeks.length === 0) {
        throw new Error('Invalid response');
      }

      const jan2026Timestamp = 1767225600;
      const weeksWithCommits = weeks
        .filter(w => w.week >= jan2026Timestamp && w.total > 0)
        .map(w => ({
          label: formatWeekLabel(w.week),
          value: w.total,
        }));

      let finalData: Array<{ label: string; value: number }>;
      let finalTotal: number;

      if (weeksWithCommits.length > 0) {
        finalData = weeksWithCommits;
        finalTotal = weeksWithCommits.reduce((sum, w) => sum + w.value, 0);
      } else {
        finalData = weeks
          .filter(w => w.total > 0)
          .slice(-12)
          .map(w => ({
            label: formatWeekLabel(w.week),
            value: w.total,
          }));
        finalTotal = finalData.reduce((sum, w) => sum + w.value, 0);
      }

      setData(finalData);
      setTotalCommits(finalTotal);
      setCachedData(finalData, finalTotal);
    } catch (e) {
      console.error('GitHub API error:', e);
      setError(e instanceof Error ? e.message : 'Failed to load');
    } finally {
      setLoading(false);
    }
  });

  return (
    <div class="mb-8 rounded-2xl border border-white/5 bg-slate-900/50 p-6">
      <div class="mb-4 flex items-center justify-between">
        <div class="flex items-center gap-2">
          <svg class="h-5 w-5 text-slate-400" fill="currentColor" viewBox="0 0 24 24">
            <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z" />
          </svg>
          <span class="text-sm font-medium text-slate-300">Weekly Commits (2026)</span>
        </div>
        <Show when={!loading() && totalCommits() > 0}>
          <span class="text-xs font-medium text-indigo-400">
            {totalCommits().toLocaleString()} commits
          </span>
        </Show>
      </div>

      <Show when={loading()}>
        <div class="flex h-32 items-center justify-center">
          <div class="h-6 w-6 animate-spin rounded-full border-2 border-indigo-500 border-t-transparent" />
        </div>
      </Show>

      <Show when={!loading() && data().length > 0}>
        <a
          href="https://github.com/PyRo1121/omg"
          target="_blank"
          rel="noopener noreferrer"
          class="block transition-opacity hover:opacity-90"
        >
          <div class="flex items-end gap-3" style={{ height: '140px' }}>
            {data().map((item) => {
              const maxVal = Math.max(...data().map(d => d.value));
              const barHeight = Math.max((item.value / maxVal) * 120, 8);
              return (
                <div class="group relative flex flex-1 flex-col items-center gap-2">
                  <div
                    class="w-full rounded-t-lg bg-gradient-to-t from-indigo-600 to-indigo-400 transition-all duration-500 group-hover:brightness-125"
                    style={{
                      height: `${barHeight}px`,
                      'box-shadow': '0 0 20px -5px rgba(99,102,241,0.4)',
                    }}
                  />
                  <span class="text-[10px] font-medium text-slate-500 group-hover:text-slate-300 transition-colors">
                    {item.label}
                  </span>
                  <div class="pointer-events-none absolute bottom-full left-1/2 z-30 mb-2 -translate-x-1/2 scale-95 rounded-xl border border-white/10 bg-slate-900/95 px-3 py-2 text-xs whitespace-nowrap text-white opacity-0 shadow-xl backdrop-blur-md transition-all duration-200 group-hover:scale-100 group-hover:opacity-100">
                    <span class="font-bold text-indigo-400">{item.value}</span> commits
                  </div>
                </div>
              );
            })}
          </div>
        </a>
      </Show>

      <Show when={!loading() && data().length === 0 && !error()}>
        <div class="flex h-32 items-center justify-center text-sm text-slate-500">
          No commit data available
        </div>
      </Show>

      <Show when={error()}>
        <div class="flex h-32 items-center justify-center text-sm text-slate-500">
          {error()}
        </div>
      </Show>
    </div>
  );
};

export default GitHubActivity;
