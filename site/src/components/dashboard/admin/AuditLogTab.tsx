import { Component, For, Show, createSignal } from 'solid-js';
import {
  Shield,
  User,
  CreditCard,
  Key,
  Settings,
  AlertTriangle,
  ArrowLeft,
  ArrowRight,
  Filter,
} from '../../ui/Icons';
import { useAdminAuditLog } from '../../../lib/api-hooks';
import { CardSkeleton } from '../../ui/Skeleton';
import { formatRelativeTime } from '../../../lib/api';

const ACTION_ICONS: Record<string, typeof Shield> = {
  'auth.login': User,
  'auth.logout': User,
  'billing.checkout_created': CreditCard,
  'billing.portal_opened': CreditCard,
  'license.regenerated': Key,
  'admin.user_updated': Settings,
  'admin.rate_limited': AlertTriangle,
};

const ACTION_COLORS: Record<string, string> = {
  'auth.login': 'text-emerald-400 bg-emerald-500/10',
  'auth.logout': 'text-slate-400 bg-slate-500/10',
  'billing.checkout_created': 'text-indigo-400 bg-indigo-500/10',
  'billing.portal_opened': 'text-indigo-400 bg-indigo-500/10',
  'license.regenerated': 'text-amber-400 bg-amber-500/10',
  'admin.user_updated': 'text-violet-400 bg-violet-500/10',
  'admin.rate_limited': 'text-rose-400 bg-rose-500/10',
};

const formatAction = (action: string) => {
  return action
    .split('.')
    .map(word => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
};

export const AuditLogTab: Component = () => {
  const [page, setPage] = createSignal(1);
  const [actionFilter, setActionFilter] = createSignal('');
  const limit = 25;

  const auditQuery = useAdminAuditLog(page(), limit, actionFilter());

  const logs = () => auditQuery.data?.logs || [];
  const pagination = () => auditQuery.data?.pagination;
  const totalPages = () => pagination()?.pages || 1;

  const getIcon = (action: string) => {
    return ACTION_ICONS[action] || Shield;
  };

  const getColor = (action: string) => {
    return ACTION_COLORS[action] || 'text-slate-400 bg-slate-500/10';
  };

  const uniqueActions = [
    'auth.login',
    'auth.logout',
    'billing.checkout_created',
    'billing.portal_opened',
    'license.regenerated',
    'admin.user_updated',
  ];

  return (
    <div class="animate-in fade-in slide-in-from-bottom-4 space-y-6 duration-500">
      <div class="rounded-3xl border border-white/5 bg-[#0d0d0e] p-8 shadow-2xl">
        <div class="mb-8 flex flex-col gap-6 md:flex-row md:items-center md:justify-between">
          <div>
            <h3 class="text-2xl font-black tracking-tight text-white">Security Audit Log</h3>
            <p class="mt-1 text-sm font-medium text-slate-500">
              Complete history of system actions and access events
            </p>
          </div>

          <div class="flex items-center gap-3">
            <div class="relative">
              <Filter size={16} class="absolute top-1/2 left-3 -translate-y-1/2 text-slate-500" />
              <select
                value={actionFilter()}
                onChange={e => {
                  setActionFilter(e.target.value);
                  setPage(1);
                }}
                class="cursor-pointer appearance-none rounded-xl border border-white/10 bg-white/5 py-2.5 pr-8 pl-10 text-sm text-white transition-all focus:ring-2 focus:ring-indigo-500/20 focus:outline-none"
              >
                <option value="">All Actions</option>
                <For each={uniqueActions}>
                  {action => <option value={action}>{formatAction(action)}</option>}
                </For>
              </select>
            </div>
          </div>
        </div>

        <Show when={auditQuery.isLoading}>
          <div class="space-y-4">
            <CardSkeleton />
            <CardSkeleton />
            <CardSkeleton />
          </div>
        </Show>

        <Show when={auditQuery.isSuccess}>
          <div class="space-y-3">
            <For each={logs()}>
              {entry => {
                const Icon = getIcon(entry.action);
                const colorClass = getColor(entry.action);
                return (
                  <div class="group relative flex items-start gap-4 rounded-2xl border border-white/5 bg-white/[0.02] p-5 transition-all hover:border-white/10 hover:bg-white/[0.04]">
                    <div class={`shrink-0 rounded-xl p-3 ${colorClass}`}>
                      <Icon size={18} />
                    </div>

                    <div class="min-w-0 flex-1">
                      <div class="flex items-start justify-between gap-4">
                        <div>
                          <p class="text-sm font-bold text-white">{formatAction(entry.action)}</p>
                          <p class="mt-1 text-xs text-slate-500">{entry.user_email || 'System'}</p>
                        </div>
                        <div class="shrink-0 text-right">
                          <p class="text-xs font-medium text-slate-400">
                            {formatRelativeTime(entry.created_at)}
                          </p>
                          <Show when={entry.ip_address}>
                            <p class="mt-1 font-mono text-[10px] text-slate-600">
                              {entry.ip_address}
                            </p>
                          </Show>
                        </div>
                      </div>

                      <Show when={entry.resource_type || entry.resource_id}>
                        <div class="mt-3 flex items-center gap-2 text-[10px] font-bold tracking-widest uppercase">
                          <Show when={entry.resource_type}>
                            <span class="rounded-lg bg-white/5 px-2 py-1 text-slate-400">
                              {entry.resource_type}
                            </span>
                          </Show>
                          <Show when={entry.resource_id}>
                            <span class="max-w-[200px] truncate font-mono text-slate-500">
                              {entry.resource_id}
                            </span>
                          </Show>
                        </div>
                      </Show>

                      <Show when={entry.metadata}>
                        <div class="mt-3 rounded-xl bg-black/20 p-3">
                          <pre class="overflow-x-auto font-mono text-[10px] text-slate-500">
                            {typeof entry.metadata === 'string'
                              ? entry.metadata
                              : JSON.stringify(entry.metadata, null, 2)}
                          </pre>
                        </div>
                      </Show>
                    </div>
                  </div>
                );
              }}
            </For>

            <Show when={logs().length === 0}>
              <div class="py-12 text-center">
                <Shield size={48} class="mx-auto mb-4 text-slate-600" />
                <p class="font-medium text-slate-500">No audit logs found</p>
                <p class="mt-1 text-xs text-slate-600">
                  {actionFilter() ? 'Try a different filter' : 'Audit events will appear here'}
                </p>
              </div>
            </Show>
          </div>

          <Show when={totalPages() > 1}>
            <div class="mt-8 flex items-center justify-between border-t border-white/5 pt-6">
              <p class="text-sm text-slate-500">
                Page {page()} of {totalPages()} ({pagination()?.total || 0} total entries)
              </p>
              <div class="flex items-center gap-2">
                <button
                  onClick={() => setPage(Math.max(1, page() - 1))}
                  disabled={page() === 1}
                  class="flex items-center gap-1 rounded-xl border border-white/10 bg-white/[0.02] px-4 py-2 text-sm font-bold text-white transition-all hover:bg-white/[0.06] disabled:cursor-not-allowed disabled:opacity-30"
                >
                  <ArrowLeft size={16} />
                  Previous
                </button>
                <button
                  onClick={() => setPage(Math.min(totalPages(), page() + 1))}
                  disabled={page() === totalPages()}
                  class="flex items-center gap-1 rounded-xl border border-white/10 bg-white/[0.02] px-4 py-2 text-sm font-bold text-white transition-all hover:bg-white/[0.06] disabled:cursor-not-allowed disabled:opacity-30"
                >
                  Next
                  <ArrowRight size={16} />
                </button>
              </div>
            </div>
          </Show>
        </Show>
      </div>
    </div>
  );
};

export default AuditLogTab;
