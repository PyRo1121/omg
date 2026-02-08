import { Component, createSignal, createResource, For, Show } from 'solid-js';
import GlassCard from '../../ui/GlassCard';
import {
  Terminal,
  Package,
  Search,
  Repeat,
  Shield,
  Zap,
  CheckCircle,
  XCircle,
  Loader2,
  AlertCircle,
  Filter,
  ChevronDown,
  ChevronUp,
  Clock,
  RefreshCw,
} from '../../ui/Icons';

interface CommandHistoryItem {
  id: string;
  command: string;
  packageName: string | null;
  runtimeName: string | null;
  success: boolean;
  durationMs: number | null;
  createdAt: string;
  machineHostname: string | null;
}

interface CommandHistoryResponse {
  commands: CommandHistoryItem[];
  pagination: {
    limit: number;
    offset: number;
    total: number;
    hasMore: boolean;
  };
  filters: {
    availableCommands: string[];
  };
}

interface Filters {
  command: string;
  startDate: string;
  endDate: string;
  success: string;
}

async function fetchCommandHistory(filters: Filters): Promise<CommandHistoryResponse> {
  const params = new URLSearchParams();
  params.set('limit', '50');
  if (filters.command) params.set('command', filters.command);
  if (filters.startDate) params.set('startDate', filters.startDate);
  if (filters.endDate) params.set('endDate', filters.endDate);
  if (filters.success) params.set('success', filters.success);

  const response = await fetch(`/api/dashboard/commands?${params}`, {
    credentials: 'include',
  });
  if (!response.ok) {
    throw new Error('Failed to fetch command history');
  }
  return response.json();
}

const getCommandIcon = (command: string) => {
  switch (command) {
    case 'install':
      return Package;
    case 'search':
      return Search;
    case 'update':
      return Repeat;
    case 'remove':
      return Package;
    case 'use':
      return Zap;
    case 'sbom':
    case 'audit':
      return Shield;
    default:
      return Terminal;
  }
};

const getCommandColor = (command: string) => {
  const colors: Record<string, string> = {
    install: 'text-indigo-400',
    search: 'text-cyan-400',
    update: 'text-emerald-400',
    remove: 'text-rose-400',
    use: 'text-purple-400',
    info: 'text-amber-400',
    list: 'text-blue-400',
    sbom: 'text-green-400',
    audit: 'text-orange-400',
  };
  return colors[command] || 'text-slate-400';
};

const formatDuration = (ms: number | null): string => {
  if (ms === null) return '-';
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60000).toFixed(1)}m`;
};

const formatRelativeTime = (dateStr: string): string => {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);

  if (diffMins < 1) return 'Just now';
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
};

export const CommandHistory: Component = () => {
  const [showFilters, setShowFilters] = createSignal(false);
  const [filters, setFilters] = createSignal<Filters>({
    command: '',
    startDate: '',
    endDate: '',
    success: '',
  });

  const [history, { refetch }] = createResource(() => filters(), fetchCommandHistory);

  const updateFilter = (key: keyof Filters, value: string) => {
    setFilters(prev => ({ ...prev, [key]: value }));
  };

  const clearFilters = () => {
    setFilters({
      command: '',
      startDate: '',
      endDate: '',
      success: '',
    });
  };

  const hasActiveFilters = () => {
    const f = filters();
    return f.command || f.startDate || f.endDate || f.success;
  };

  return (
    <div class="space-y-6">
      <GlassCard class="p-6">
        {/* Header */}
        <div class="flex items-center justify-between mb-6">
          <div>
            <h2 class="text-xl font-bold text-white">Command History</h2>
            <p class="text-sm text-slate-400">
              Your recent CLI activity
              <Show when={history()?.pagination}>
                {' '}- {history()!.pagination.total.toLocaleString()} total commands
              </Show>
            </p>
          </div>
          <div class="flex items-center gap-2">
            <button
              onClick={() => refetch()}
              class="p-2 rounded-lg bg-white/5 border border-white/10 text-slate-400 hover:text-white transition-all"
              title="Refresh"
            >
              <RefreshCw size={16} class={history.loading ? 'animate-spin' : ''} />
            </button>
            <button
              onClick={() => setShowFilters(!showFilters())}
              class={`flex items-center gap-2 px-3 py-2 rounded-lg border transition-all ${
                hasActiveFilters()
                  ? 'bg-indigo-500/20 border-indigo-500/30 text-indigo-400'
                  : 'bg-white/5 border-white/10 text-slate-400 hover:text-white'
              }`}
            >
              <Filter size={16} />
              <span class="text-sm">Filters</span>
              <Show when={hasActiveFilters()}>
                <span class="px-1.5 py-0.5 rounded bg-indigo-500/30 text-xs">
                  {Object.values(filters()).filter(Boolean).length}
                </span>
              </Show>
              <Show when={showFilters()} fallback={<ChevronDown size={14} />}>
                <ChevronUp size={14} />
              </Show>
            </button>
          </div>
        </div>

        {/* Filters Panel */}
        <Show when={showFilters()}>
          <div class="mb-6 p-4 rounded-xl bg-white/5 border border-white/10">
            <div class="grid grid-cols-1 md:grid-cols-4 gap-4">
              <div>
                <label class="block text-xs font-medium text-slate-400 mb-1.5">
                  Command Type
                </label>
                <select
                  value={filters().command}
                  onChange={(e) => updateFilter('command', e.currentTarget.value)}
                  class="w-full px-3 py-2 rounded-lg bg-slate-900/50 border border-white/10 text-white text-sm focus:border-indigo-500 focus:outline-none"
                >
                  <option value="">All commands</option>
                  <For each={history()?.filters.availableCommands || []}>
                    {(cmd) => (
                      <option value={cmd}>{cmd}</option>
                    )}
                  </For>
                </select>
              </div>

              <div>
                <label class="block text-xs font-medium text-slate-400 mb-1.5">
                  From Date
                </label>
                <input
                  type="date"
                  value={filters().startDate}
                  onChange={(e) => updateFilter('startDate', e.currentTarget.value)}
                  class="w-full px-3 py-2 rounded-lg bg-slate-900/50 border border-white/10 text-white text-sm focus:border-indigo-500 focus:outline-none"
                />
              </div>

              <div>
                <label class="block text-xs font-medium text-slate-400 mb-1.5">
                  To Date
                </label>
                <input
                  type="date"
                  value={filters().endDate}
                  onChange={(e) => updateFilter('endDate', e.currentTarget.value)}
                  class="w-full px-3 py-2 rounded-lg bg-slate-900/50 border border-white/10 text-white text-sm focus:border-indigo-500 focus:outline-none"
                />
              </div>

              <div>
                <label class="block text-xs font-medium text-slate-400 mb-1.5">
                  Status
                </label>
                <select
                  value={filters().success}
                  onChange={(e) => updateFilter('success', e.currentTarget.value)}
                  class="w-full px-3 py-2 rounded-lg bg-slate-900/50 border border-white/10 text-white text-sm focus:border-indigo-500 focus:outline-none"
                >
                  <option value="">All</option>
                  <option value="true">Success</option>
                  <option value="false">Failed</option>
                </select>
              </div>
            </div>

            <Show when={hasActiveFilters()}>
              <div class="mt-4 flex justify-end">
                <button
                  onClick={clearFilters}
                  class="text-sm text-slate-400 hover:text-white transition-colors"
                >
                  Clear all filters
                </button>
              </div>
            </Show>
          </div>
        </Show>

        {/* Loading State */}
        <Show when={history.loading}>
          <div class="flex items-center justify-center py-12">
            <Loader2 size={32} class="animate-spin text-indigo-400" />
          </div>
        </Show>

        {/* Error State */}
        <Show when={history.error}>
          <div class="flex items-center gap-3 p-4 rounded-xl bg-rose-500/10 border border-rose-500/20 text-rose-400">
            <AlertCircle size={20} />
            <span>Failed to load command history. Please try again.</span>
          </div>
        </Show>

        {/* Command List */}
        <Show when={history() && !history.loading}>
          <Show when={history()!.commands.length === 0}>
            <div class="text-center py-12">
              <Terminal size={48} class="mx-auto text-slate-600 mb-4" />
              <p class="text-slate-400">No commands found</p>
              <Show when={hasActiveFilters()}>
                <button
                  onClick={clearFilters}
                  class="mt-2 text-sm text-indigo-400 hover:text-indigo-300"
                >
                  Clear filters to see all commands
                </button>
              </Show>
            </div>
          </Show>

          <Show when={history()!.commands.length > 0}>
            <div class="overflow-x-auto">
              <table class="w-full">
                <thead>
                  <tr class="border-b border-white/10">
                    <th class="px-4 py-3 text-left text-xs font-bold uppercase tracking-wider text-slate-500">
                      Command
                    </th>
                    <th class="px-4 py-3 text-left text-xs font-bold uppercase tracking-wider text-slate-500">
                      Target
                    </th>
                    <th class="px-4 py-3 text-left text-xs font-bold uppercase tracking-wider text-slate-500">
                      Status
                    </th>
                    <th class="px-4 py-3 text-left text-xs font-bold uppercase tracking-wider text-slate-500">
                      Duration
                    </th>
                    <th class="px-4 py-3 text-left text-xs font-bold uppercase tracking-wider text-slate-500">
                      Time
                    </th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-white/5">
                  <For each={history()!.commands}>
                    {(cmd) => {
                      const Icon = getCommandIcon(cmd.command);
                      const colorClass = getCommandColor(cmd.command);
                      return (
                        <tr class="hover:bg-white/5 transition-colors">
                          <td class="px-4 py-3">
                            <div class="flex items-center gap-2">
                              <Icon size={16} class={colorClass} />
                              <span class={`font-medium capitalize ${colorClass}`}>
                                {cmd.command}
                              </span>
                            </div>
                          </td>
                          <td class="px-4 py-3">
                            <span class="text-sm text-slate-300 font-mono">
                              {cmd.packageName || cmd.runtimeName || '-'}
                            </span>
                          </td>
                          <td class="px-4 py-3">
                            <Show when={cmd.success} fallback={
                              <div class="flex items-center gap-1.5 text-rose-400">
                                <XCircle size={14} />
                                <span class="text-xs font-medium">Failed</span>
                              </div>
                            }>
                              <div class="flex items-center gap-1.5 text-emerald-400">
                                <CheckCircle size={14} />
                                <span class="text-xs font-medium">Success</span>
                              </div>
                            </Show>
                          </td>
                          <td class="px-4 py-3">
                            <div class="flex items-center gap-1.5 text-slate-400">
                              <Clock size={12} />
                              <span class="text-sm font-mono">
                                {formatDuration(cmd.durationMs)}
                              </span>
                            </div>
                          </td>
                          <td class="px-4 py-3">
                            <span class="text-sm text-slate-500" title={cmd.createdAt}>
                              {formatRelativeTime(cmd.createdAt)}
                            </span>
                          </td>
                        </tr>
                      );
                    }}
                  </For>
                </tbody>
              </table>
            </div>

            {/* Pagination Info */}
            <Show when={history()!.pagination.hasMore}>
              <div class="mt-4 text-center">
                <p class="text-sm text-slate-500">
                  Showing {history()!.commands.length} of {history()!.pagination.total.toLocaleString()} commands
                </p>
              </div>
            </Show>
          </Show>
        </Show>
      </GlassCard>
    </div>
  );
};

export default CommandHistory;
