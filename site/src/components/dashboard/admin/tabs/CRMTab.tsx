import { Component, createSignal, Show, For } from 'solid-js';
import { Search, Users } from 'lucide-solid';
import { debounce } from '@solid-primitives/scheduled';
import { CardSkeleton } from '../../../ui/Skeleton';
import ErrorCard from '../shared/ErrorCard';
import {
  CRMProfileCard,
  CRMProfileCardTableRow,
} from '../../premium';
import type { CRMCustomer } from '../../premium/types';

interface CRMTabProps {
  customers: CRMCustomer[];
  pagination: { total: number; pages: number; page: number; limit: number } | undefined;
  isLoading: boolean;
  isSuccess: boolean;
  isError?: boolean;
  onSearchChange: (search: string) => void;
  onPageChange: (page: number) => void;
  onViewDetail: (customerId: string) => void;
  onRetry?: () => void;
}

export const CRMTab: Component<CRMTabProps> = (props) => {
  const [viewMode, setViewMode] = createSignal<'table' | 'cards'>('cards');
  const [search, setSearch] = createSignal('');

  const isMobile = () => typeof window !== 'undefined' && window.innerWidth < 768;
  const effectiveViewMode = () => isMobile() ? 'cards' : viewMode();

  const debouncedSearch = debounce((value: string) => {
    props.onSearchChange(value);
  }, 300);

  const handleSearchInput = (value: string) => {
    setSearch(value);
    debouncedSearch(value);
  };

  return (
    <div class="space-y-6">
      <div class="flex flex-col justify-between gap-6 md:flex-row md:items-center">
        <div>
          <h3 class="text-2xl font-black tracking-tight text-white">Customer CRM</h3>
          <p class="text-sm font-medium text-slate-500">
            {props.pagination?.total || 0} customers | Manage subscriptions and engagement
          </p>
        </div>

        <div class="flex flex-col gap-4 sm:flex-row sm:items-center">
          <div class="hidden rounded-xl border border-white/10 bg-white/[0.02] p-1 md:flex">
            <button
              onClick={() => setViewMode('table')}
              class={`rounded-lg px-4 py-2 text-xs font-bold transition-all ${
                viewMode() === 'table'
                  ? 'bg-white text-black'
                  : 'text-slate-400 hover:text-white'
              }`}
            >
              Table
            </button>
            <button
              onClick={() => setViewMode('cards')}
              class={`rounded-lg px-4 py-2 text-xs font-bold transition-all ${
                viewMode() === 'cards'
                  ? 'bg-white text-black'
                  : 'text-slate-400 hover:text-white'
              }`}
            >
              Cards
            </button>
          </div>

          <div class="relative w-full sm:max-w-md">
            <Search class="absolute left-4 top-1/2 -translate-y-1/2 text-slate-500" size={18} />
            <input
              type="text"
              placeholder="Search by email, company or ID..."
              value={search()}
              onInput={(e) => handleSearchInput(e.currentTarget.value)}
              class="w-full rounded-2xl border border-white/10 bg-white/5 py-3 pl-12 pr-4 text-white placeholder-slate-500 transition-all focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
            />
          </div>
        </div>
      </div>

      <Show when={props.isLoading}>
        <div class="grid gap-6 md:grid-cols-2 xl:grid-cols-3">
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
        </div>
      </Show>

      <Show when={props.isError}>
        <ErrorCard
          title="Failed to Load Customers"
          message="Unable to fetch customer data. Please check your connection and try again."
          onRetry={props.onRetry}
        />
      </Show>

      <Show when={props.isSuccess}>
        <Show when={effectiveViewMode() === 'cards'}>
          <div class="grid gap-6 md:grid-cols-2 xl:grid-cols-3">
            <For each={props.customers}>
              {(customer, index) => (
                <div
                  class="animate-fade-in-up"
                  style={{
                    'animation-delay': `${index() * 50}ms`,
                  }}
                >
                  <CRMProfileCard
                    customer={customer}
                    onViewDetail={(customerId) => props.onViewDetail(customerId)}
                    onQuickAction={(action, _customerId) => {
                      if (action === 'email') {
                        window.open(`mailto:${customer.email}`);
                      }
                    }}
                  />
                </div>
              )}
            </For>
          </div>
        </Show>

        <Show when={effectiveViewMode() === 'table'}>
          <div class="overflow-hidden rounded-[2rem] border border-white/5 bg-[#0d0d0e] shadow-2xl">
            <div class="overflow-x-auto">
              <table class="w-full text-left">
                <thead>
                  <tr class="border-b border-white/5 text-[10px] font-black uppercase tracking-widest text-slate-500">
                    <th class="px-6 py-4">User</th>
                    <th class="px-6 py-4">Tier</th>
                    <th class="px-6 py-4">Status</th>
                    <th class="px-6 py-4">Health</th>
                    <th class="px-6 py-4">Machines</th>
                    <th class="px-6 py-4">Commands</th>
                    <th class="px-6 py-4">Joined</th>
                    <th class="px-6 py-4"></th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-white/5">
                  <For each={props.customers}>
                    {(customer) => (
                      <CRMProfileCardTableRow
                        customer={customer}
                        onViewDetail={(customerId) => props.onViewDetail(customerId)}
                        onQuickAction={(action, _customerId) => {
                          if (action === 'email') {
                            window.open(`mailto:${customer.email}`);
                          }
                        }}
                      />
                    )}
                  </For>
                </tbody>
              </table>
            </div>

            <Show when={props.customers.length === 0}>
              <div class="py-12 text-center">
                <Users size={48} class="mx-auto mb-4 text-slate-600" />
                <p class="font-medium text-slate-500">No customers found</p>
                <p class="mt-1 text-xs text-slate-600">
                  {search() ? 'Try a different search term' : 'Customers will appear here'}
                </p>
              </div>
            </Show>

            <Show when={(props.pagination?.pages || 1) > 1}>
              <div class="flex items-center justify-between border-t border-white/5 px-6 py-4">
                <p class="text-sm text-slate-500">
                  Page {props.pagination?.page || 1} of {props.pagination?.pages || 1}
                </p>
                <div class="flex items-center gap-2">
                  <button
                    onClick={() => props.onPageChange(Math.max(1, (props.pagination?.page || 1) - 1))}
                    disabled={(props.pagination?.page || 1) === 1}
                    class="rounded-xl border border-white/10 bg-white/[0.02] px-4 py-2 text-sm font-bold text-white transition-all hover:bg-white/[0.06] disabled:cursor-not-allowed disabled:opacity-30"
                  >
                    Previous
                  </button>
                  <button
                    onClick={() => props.onPageChange(Math.min(props.pagination?.pages || 1, (props.pagination?.page || 1) + 1))}
                    disabled={(props.pagination?.page || 1) === (props.pagination?.pages || 1)}
                    class="rounded-xl border border-white/10 bg-white/[0.02] px-4 py-2 text-sm font-bold text-white transition-all hover:bg-white/[0.06] disabled:cursor-not-allowed disabled:opacity-30"
                  >
                    Next
                  </button>
                </div>
              </div>
            </Show>
          </div>
        </Show>
      </Show>
    </div>
  );
};
