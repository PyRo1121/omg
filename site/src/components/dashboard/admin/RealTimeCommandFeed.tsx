import { Component, For, Show, createSignal, createEffect, createMemo, onCleanup } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import {
  Terminal,
  Package,
  Search,
  RefreshCw,
  AlertCircle,
  CheckCircle,
  Filter,
  Pause,
  Play,
  Trash2,
  Clock,
  User,
  Shield,
} from 'lucide-solid';
import type { CommandEvent } from '../../../hooks/useRealtimeData';
import { LiveIndicator } from '../../../design-system';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// ============================================================================
// Types
// ============================================================================

interface RealTimeCommandFeedProps {
  commands: CommandEvent[];
  isLive?: boolean;
  maxVisible?: number;
  onClear?: () => void;
  class?: string;
}

type CommandFilter = 'all' | 'install' | 'search' | 'update' | 'remove' | 'info';
type StatusFilter = 'all' | 'success' | 'error';
type TierFilter = 'all' | 'free' | 'pro' | 'team' | 'enterprise';

// ============================================================================
// Constants
// ============================================================================

const COMMAND_TYPE_CONFIG: Record<string, {
  icon: typeof Terminal;
  color: string;
  bg: string;
  label: string
}> = {
  install: { icon: Package, color: 'text-aurora-400', bg: 'bg-aurora-500/10', label: 'INSTALL' },
  search: { icon: Search, color: 'text-electric-400', bg: 'bg-electric-500/10', label: 'SEARCH' },
  update: { icon: RefreshCw, color: 'text-photon-400', bg: 'bg-photon-500/10', label: 'UPDATE' },
  remove: { icon: Trash2, color: 'text-flare-400', bg: 'bg-flare-500/10', label: 'REMOVE' },
  info: { icon: Terminal, color: 'text-plasma-400', bg: 'bg-plasma-500/10', label: 'INFO' },
  default: { icon: Terminal, color: 'text-indigo-400', bg: 'bg-indigo-500/10', label: 'CMD' },
};

const TIER_CONFIG: Record<string, { color: string; bg: string; border: string }> = {
  free: { color: 'text-nebula-400', bg: 'bg-nebula-500/10', border: 'border-nebula-500/20' },
  pro: { color: 'text-electric-400', bg: 'bg-electric-500/10', border: 'border-electric-500/20' },
  team: { color: 'text-photon-400', bg: 'bg-photon-500/10', border: 'border-photon-500/20' },
  enterprise: { color: 'text-solar-400', bg: 'bg-solar-500/10', border: 'border-solar-500/20' },
};

// ============================================================================
// Helper Functions
// ============================================================================

const getCommandType = (command: string): string => {
  const cmd = command.toLowerCase();
  if (cmd.includes('install') || cmd.includes('add')) return 'install';
  if (cmd.includes('search') || cmd.includes('find')) return 'search';
  if (cmd.includes('update') || cmd.includes('upgrade')) return 'update';
  if (cmd.includes('remove') || cmd.includes('uninstall')) return 'remove';
  if (cmd.includes('info') || cmd.includes('show')) return 'info';
  return 'default';
};

const formatTimestamp = (timestamp: string): string => {
  const date = new Date(timestamp);
  return date.toLocaleTimeString('en-US', {
    hour12: false,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
};

const formatDuration = (ms: number): string => {
  if (ms < 1) return '<1ms';
  if (ms < 1000) return `${Math.round(ms)}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
};

const formatLicenseKey = (key: string): string => {
  if (!key || key.length < 8) return key;
  return `${key.slice(0, 4)}...${key.slice(-4)}`;
};

// ============================================================================
// Sub-Components
// ============================================================================

interface CommandItemProps {
  command: CommandEvent;
  index: number;
}

const CommandItem: Component<CommandItemProps> = (props) => {
  const commandType = createMemo(() => getCommandType(props.command.command));
  const config = createMemo(() => COMMAND_TYPE_CONFIG[commandType()] || COMMAND_TYPE_CONFIG.default);
  const tierConfig = createMemo(() => TIER_CONFIG[props.command.license_tier] || TIER_CONFIG.free);

  return (
    <div
      class={cn(
        'group flex items-start gap-3 border-l-2 py-2.5 pl-3 pr-2',
        'transition-all duration-200',
        props.command.status === 'success'
          ? 'border-transparent hover:border-aurora-500 hover:bg-aurora-500/[0.02]'
          : 'border-transparent hover:border-flare-500 hover:bg-flare-500/[0.02]',
        'animate-in fade-in slide-in-from-top-1 duration-300'
      )}
      style={{ 'animation-delay': `${props.index * 20}ms` }}
    >
      {/* Timestamp */}
      <span class="shrink-0 font-mono text-2xs tabular-nums text-nebula-600">
        {formatTimestamp(props.command.timestamp)}
      </span>

      {/* Command Type Icon */}
      <div class={cn('shrink-0 rounded-lg p-1.5', config().bg)}>
        {(() => {
          const Icon = config().icon;
          return <Icon size={12} class={config().color} />;
        })()}
      </div>

      {/* Main Content */}
      <div class="min-w-0 flex-1">
        <div class="flex items-center gap-2 flex-wrap">
          {/* Command Type Label */}
          <span class={cn('text-2xs font-black uppercase tracking-wider', config().color)}>
            {config().label}
          </span>

          {/* Command */}
          <span class="truncate font-mono text-xs font-medium text-white">
            {props.command.command}
          </span>

          {/* Package Name */}
          <Show when={props.command.package_name}>
            <span class="rounded-md bg-white/5 px-1.5 py-0.5 font-mono text-2xs text-nebula-300">
              {props.command.package_name}
            </span>
          </Show>
        </div>

        {/* Metadata Row */}
        <div class="mt-1 flex items-center gap-2 flex-wrap text-2xs text-nebula-500">
          {/* License/User */}
          <div class="flex items-center gap-1">
            <User size={10} class="text-nebula-600" />
            <span class={cn('font-medium', tierConfig().color)}>
              {props.command.user_email || formatLicenseKey(props.command.license_key)}
            </span>
          </div>

          <span class="text-nebula-700">|</span>

          {/* Tier Badge */}
          <span class={cn(
            'rounded-full px-1.5 py-0.5 text-2xs font-bold uppercase',
            tierConfig().bg,
            tierConfig().color
          )}>
            {props.command.license_tier}
          </span>

          <span class="text-nebula-700">|</span>

          {/* Platform */}
          <span>{props.command.platform}</span>

          <span class="text-nebula-700">|</span>

          {/* Duration */}
          <div class="flex items-center gap-1">
            <Clock size={10} />
            <span class={props.command.duration_ms < 100 ? 'text-aurora-400' : props.command.duration_ms < 1000 ? 'text-solar-400' : 'text-flare-400'}>
              {formatDuration(props.command.duration_ms)}
            </span>
          </div>

          {/* Hostname */}
          <Show when={props.command.hostname}>
            <span class="text-nebula-700">|</span>
            <span class="truncate max-w-[100px]">{props.command.hostname}</span>
          </Show>
        </div>

        {/* Error Message */}
        <Show when={props.command.status === 'error' && props.command.error_message}>
          <div class="mt-1.5 flex items-start gap-1.5 rounded-md bg-flare-500/10 px-2 py-1">
            <AlertCircle size={12} class="text-flare-400 shrink-0 mt-0.5" />
            <span class="text-2xs text-flare-300 font-mono">{props.command.error_message}</span>
          </div>
        </Show>
      </div>

      {/* Status Indicator */}
      <div class="shrink-0">
        {props.command.status === 'success' ? (
          <CheckCircle size={14} class="text-aurora-400" />
        ) : (
          <AlertCircle size={14} class="text-flare-400" />
        )}
      </div>
    </div>
  );
};

// ============================================================================
// Filter Bar Component
// ============================================================================

interface FilterBarProps {
  commandFilter: CommandFilter;
  statusFilter: StatusFilter;
  tierFilter: TierFilter;
  onCommandFilterChange: (filter: CommandFilter) => void;
  onStatusFilterChange: (filter: StatusFilter) => void;
  onTierFilterChange: (filter: TierFilter) => void;
}

const FilterBar: Component<FilterBarProps> = (props) => {
  return (
    <div class="flex items-center gap-3 flex-wrap">
      {/* Command Type Filter */}
      <div class="flex items-center gap-1.5">
        <Filter size={12} class="text-nebula-500" />
        <select
          value={props.commandFilter}
          onChange={(e) => props.onCommandFilterChange(e.currentTarget.value as CommandFilter)}
          class="rounded-lg border border-white/10 bg-white/5 px-2 py-1 text-2xs font-bold text-white focus:border-indigo-500 focus:outline-none"
        >
          <option value="all">All Commands</option>
          <option value="install">Install</option>
          <option value="search">Search</option>
          <option value="update">Update</option>
          <option value="remove">Remove</option>
          <option value="info">Info</option>
        </select>
      </div>

      {/* Status Filter */}
      <select
        value={props.statusFilter}
        onChange={(e) => props.onStatusFilterChange(e.currentTarget.value as StatusFilter)}
        class="rounded-lg border border-white/10 bg-white/5 px-2 py-1 text-2xs font-bold text-white focus:border-indigo-500 focus:outline-none"
      >
        <option value="all">All Status</option>
        <option value="success">Success</option>
        <option value="error">Errors</option>
      </select>

      {/* Tier Filter */}
      <select
        value={props.tierFilter}
        onChange={(e) => props.onTierFilterChange(e.currentTarget.value as TierFilter)}
        class="rounded-lg border border-white/10 bg-white/5 px-2 py-1 text-2xs font-bold text-white focus:border-indigo-500 focus:outline-none"
      >
        <option value="all">All Tiers</option>
        <option value="free">Free</option>
        <option value="pro">Pro</option>
        <option value="team">Team</option>
        <option value="enterprise">Enterprise</option>
      </select>
    </div>
  );
};

// ============================================================================
// Main Component
// ============================================================================

export const RealTimeCommandFeed: Component<RealTimeCommandFeedProps> = (props) => {
  let feedRef: HTMLDivElement | undefined;

  // State
  const [isPaused, setIsPaused] = createSignal(false);
  const [isHovering, setIsHovering] = createSignal(false);
  const [commandFilter, setCommandFilter] = createSignal<CommandFilter>('all');
  const [statusFilter, setStatusFilter] = createSignal<StatusFilter>('all');
  const [tierFilter, setTierFilter] = createSignal<TierFilter>('all');

  const maxVisible = () => props.maxVisible || 100;

  // Filter commands
  const filteredCommands = createMemo(() => {
    let filtered = props.commands;

    // Apply command type filter
    if (commandFilter() !== 'all') {
      filtered = filtered.filter((cmd) => getCommandType(cmd.command) === commandFilter());
    }

    // Apply status filter
    if (statusFilter() !== 'all') {
      filtered = filtered.filter((cmd) => cmd.status === statusFilter());
    }

    // Apply tier filter
    if (tierFilter() !== 'all') {
      filtered = filtered.filter((cmd) => cmd.license_tier === tierFilter());
    }

    return filtered.slice(0, maxVisible());
  });

  // Auto-scroll to top on new commands (unless paused or hovering)
  createEffect(() => {
    const commands = props.commands;
    if (feedRef && commands.length > 0 && !isPaused() && !isHovering()) {
      feedRef.scrollTop = 0;
    }
  });

  // Stats
  const successCount = createMemo(() => props.commands.filter((c) => c.status === 'success').length);
  const errorCount = createMemo(() => props.commands.filter((c) => c.status === 'error').length);
  const successRate = createMemo(() => {
    const total = props.commands.length;
    return total > 0 ? ((successCount() / total) * 100).toFixed(1) : '100';
  });

  return (
    <div
      class={cn(
        'relative overflow-hidden rounded-2xl border border-white/[0.06] bg-void-900 shadow-2xl',
        props.class
      )}
    >
      {/* Ambient glow */}
      <div class="pointer-events-none absolute -left-32 -top-32 h-64 w-64 rounded-full bg-indigo-500/10 blur-[100px]" />

      {/* Header */}
      <div class="relative border-b border-white/5 bg-gradient-to-r from-white/[0.02] to-transparent p-4">
        <div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
          {/* Title & Live Indicator */}
          <div class="flex items-center gap-3">
            <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500 to-photon-600">
              <Terminal size={20} class="text-white" />
            </div>
            <div>
              <h2 class="font-display text-lg font-black tracking-tight text-white">
                Live Command Feed
              </h2>
              <p class="text-2xs text-nebula-500">
                Real-time CLI command stream
              </p>
            </div>

            <LiveIndicator
              label={props.isLive && !isPaused() ? 'Live' : 'Paused'}
              color={props.isLive && !isPaused() ? 'success' : 'warning'}
              variant="pulse"
              size="sm"
            />
          </div>

          {/* Controls */}
          <div class="flex items-center gap-3">
            <FilterBar
              commandFilter={commandFilter()}
              statusFilter={statusFilter()}
              tierFilter={tierFilter()}
              onCommandFilterChange={setCommandFilter}
              onStatusFilterChange={setStatusFilter}
              onTierFilterChange={setTierFilter}
            />

            {/* Pause/Play Button */}
            <button
              onClick={() => setIsPaused(!isPaused())}
              class={cn(
                'flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-2xs font-bold transition-colors',
                isPaused()
                  ? 'bg-aurora-500/10 text-aurora-400 hover:bg-aurora-500/20'
                  : 'bg-solar-500/10 text-solar-400 hover:bg-solar-500/20'
              )}
            >
              {isPaused() ? (
                <>
                  <Play size={12} />
                  Resume
                </>
              ) : (
                <>
                  <Pause size={12} />
                  Pause
                </>
              )}
            </button>

            {/* Clear Button */}
            <Show when={props.onClear}>
              <button
                onClick={props.onClear}
                class="flex items-center gap-1.5 rounded-lg bg-flare-500/10 px-2.5 py-1.5 text-2xs font-bold text-flare-400 transition-colors hover:bg-flare-500/20"
              >
                <Trash2 size={12} />
                Clear
              </button>
            </Show>
          </div>
        </div>

        {/* Stats Bar */}
        <div class="mt-3 flex items-center gap-4 text-2xs">
          <div class="flex items-center gap-1.5">
            <span class="text-nebula-500">Total:</span>
            <span class="font-bold text-white">{props.commands.length}</span>
          </div>
          <div class="flex items-center gap-1.5">
            <CheckCircle size={10} class="text-aurora-400" />
            <span class="font-bold text-aurora-400">{successCount()}</span>
          </div>
          <div class="flex items-center gap-1.5">
            <AlertCircle size={10} class="text-flare-400" />
            <span class="font-bold text-flare-400">{errorCount()}</span>
          </div>
          <div class="flex items-center gap-1.5">
            <Shield size={10} class="text-nebula-500" />
            <span class="text-nebula-400">
              <span class="font-bold text-white">{successRate()}%</span> success rate
            </span>
          </div>
        </div>
      </div>

      {/* Command Stream */}
      <div
        ref={feedRef}
        class="relative h-[400px] overflow-y-auto font-mono text-xs"
        style={{ 'scrollbar-width': 'none', '-ms-overflow-style': 'none' }}
        onMouseEnter={() => setIsHovering(true)}
        onMouseLeave={() => setIsHovering(false)}
      >
        {/* Sticky Header */}
        <div class="sticky top-0 z-10 border-b border-white/5 bg-void-900/95 px-4 py-2 backdrop-blur-xl">
          <div class="flex items-center gap-2 text-2xs font-bold uppercase tracking-wider text-nebula-600">
            <Terminal size={12} />
            <span>Command Stream</span>
            <span class="ml-auto text-nebula-500">{filteredCommands().length} visible</span>
          </div>
        </div>

        {/* Commands List */}
        <div class="divide-y divide-white/[0.02] px-2">
          <For each={filteredCommands()}>
            {(command, i) => <CommandItem command={command} index={i()} />}
          </For>
        </div>

        {/* Empty State */}
        <Show when={filteredCommands().length === 0}>
          <div class="flex flex-col items-center justify-center py-16 text-center">
            <Terminal size={48} class="mb-4 text-nebula-600" />
            <p class="font-sans text-sm font-bold text-nebula-500">No commands to display</p>
            <p class="mt-1 font-sans text-xs text-nebula-600">
              {commandFilter() !== 'all' || statusFilter() !== 'all' || tierFilter() !== 'all'
                ? 'Try adjusting your filters'
                : 'Waiting for incoming commands...'}
            </p>
          </div>
        </Show>

        {/* Streaming Indicator */}
        <div class="sticky bottom-0 flex items-center gap-2 border-t border-white/5 bg-void-900/95 px-4 py-2 backdrop-blur-xl">
          <Show
            when={!isPaused()}
            fallback={
              <>
                <div class="h-1.5 w-1.5 rounded-full bg-solar-500" />
                <span class="text-2xs italic text-nebula-600">Stream paused - hover to scroll, click Resume to continue</span>
              </>
            }
          >
            <div class="h-1.5 w-1.5 animate-pulse rounded-full bg-aurora-500" />
            <span class="text-2xs italic text-nebula-600">Streaming live command data...</span>
          </Show>
        </div>
      </div>
    </div>
  );
};

export default RealTimeCommandFeed;
