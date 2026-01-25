import { Component, createMemo, For, Show } from 'solid-js';
import {
  createSolidTable,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  getPaginationRowModel,
  ColumnDef,
  SortingState,
} from '@tanstack/solid-table';
import { createSignal } from 'solid-js';
import * as api from '../../../lib/api';
import GlassCard from '../../ui/GlassCard';
import { ChevronUp, ChevronDown, Trash2, Monitor } from '../../ui/Icons';

interface FleetTableProps {
  data: api.Machine[];
  onRevoke: (id: string) => void;
}

export const FleetTable: Component<FleetTableProps> = (props) => {
  const [sorting, setSorting] = createSignal<SortingState>([]);

  const columns: ColumnDef<api.Machine>[] = [
    {
      accessorKey: 'hostname',
      header: 'Hostname',
      cell: (info) => (
        <div class="flex items-center gap-3">
          <div class="p-2 rounded-lg bg-white/5 text-slate-400">
            <Monitor size={16} />
          </div>
          <span class="font-bold text-white">{info.getValue<string>() || 'Unknown'}</span>
        </div>
      ),
    },
    {
      accessorKey: 'os',
      header: 'OS / Arch',
      cell: (info) => (
        <div class="flex flex-col">
          <span class="text-sm text-slate-300">{info.getValue<string>() || 'Unknown'}</span>
          <span class="text-[10px] font-mono text-slate-500 uppercase">{info.row.original.arch || 'x64'}</span>
        </div>
      ),
    },
    {
      accessorKey: 'omg_version',
      header: 'Version',
      cell: (info) => (
        <span class="px-2 py-0.5 rounded bg-indigo-500/10 text-indigo-400 font-mono text-xs border border-indigo-500/20">
          v{info.getValue<string>() || '0.0.0'}
        </span>
      ),
    },
    {
      accessorKey: 'last_seen_at',
      header: 'Status',
      cell: (info) => {
        const lastSeen = new Date(info.getValue<string>());
        const isOnline = new Date().getTime() - lastSeen.getTime() < 300000; // 5 mins
        return (
          <div class="flex items-center gap-2">
            <div class={`h-2 w-2 rounded-full ${isOnline ? 'bg-emerald-500 animate-pulse' : 'bg-slate-600'}`} />
            <span class="text-xs font-medium text-slate-400">
              {isOnline ? 'Online' : api.formatRelativeTime(info.getValue<string>())}
            </span>
          </div>
        );
      },
    },
    {
      id: 'actions',
      header: '',
      cell: (info) => (
        <button
          onClick={() => props.onRevoke(info.row.original.machine_id)}
          class="p-2 rounded-lg hover:bg-rose-500/10 text-slate-500 hover:text-rose-400 transition-colors"
        >
          <Trash2 size={18} />
        </button>
      ),
    },
  ];

  const table = createSolidTable({
    get data() { return props.data },
    columns,
    state: {
      get sorting() { return sorting() },
    },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
  });

  return (
    <div class="space-y-4">
      <div class="overflow-x-auto rounded-2xl border border-white/5 bg-white/[0.02]">
        <table class="w-full text-left border-collapse">
          <thead>
            <For each={table.getHeaderGroups()}>
              {(headerGroup) => (
                <tr class="border-b border-white/5 bg-white/[0.02]">
                  <For each={headerGroup.headers}>
                    {(header) => (
                      <th class="px-6 py-4 text-[10px] font-black uppercase tracking-widest text-slate-500">
                        <Show
                          when={!header.isPlaceholder}
                          fallback={null}
                        >
                          <div
                            class={header.column.getCanSort() ? 'cursor-pointer select-none flex items-center gap-2 hover:text-white transition-colors' : ''}
                            onClick={header.column.getToggleSortingHandler()}
                          >
                            {flexRender(header.column.columnDef.header, header.getContext())}
                            <Show when={header.column.getIsSorted()}>
                              {header.column.getIsSorted() === 'asc' ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
                            </Show>
                          </div>
                        </Show>
                      </th>
                    )}
                  </For>
                </tr>
              )}
            </For>
          </thead>
          <tbody>
            <For each={table.getRowModel().rows}>
              {(row) => (
                <tr class="border-b border-white/5 hover:bg-white/[0.01] transition-colors group">
                  <For each={row.getVisibleCells()}>
                    {(cell) => (
                      <td class="px-6 py-4">
                        {flexRender(cell.column.columnDef.cell, cell.getContext())}
                      </td>
                    )}
                  </For>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </div>

      <div class="flex items-center justify-between px-2">
        <div class="flex items-center gap-2">
          <button
            class="rounded-xl border border-white/10 bg-white/[0.03] px-4 py-2 text-xs font-bold text-white transition-all hover:bg-white/[0.08] disabled:opacity-30"
            onClick={() => table.previousPage()}
            disabled={!table.getCanPreviousPage()}
          >
            Previous
          </button>
          <button
            class="rounded-xl border border-white/10 bg-white/[0.03] px-4 py-2 text-xs font-bold text-white transition-all hover:bg-white/[0.08] disabled:opacity-30"
            onClick={() => table.nextPage()}
            disabled={!table.getCanNextPage()}
          >
            Next
          </button>
        </div>
        <div class="text-[10px] font-black uppercase tracking-widest text-slate-500">
          Page {table.getState().pagination.pageIndex + 1} of {table.getPageCount()}
        </div>
      </div>
    </div>
  );
};
