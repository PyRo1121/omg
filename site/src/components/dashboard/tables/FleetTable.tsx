import { Component, createMemo, For, Show, createSignal } from 'solid-js';
import {
  createSolidTable,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  getPaginationRowModel,
  getFilteredRowModel,
  ColumnDef,
  SortingState,
  RowSelectionState,
} from '@tanstack/solid-table';
import * as api from '../../../lib/api';
import { ChevronUp, ChevronDown, Trash2, Monitor, Search, X } from '../../ui/Icons';
import { StatusBadge } from '../../ui/Badge';

interface FleetTableProps {
  data: api.Machine[];
  onRevoke: (ids: string[]) => void;
}

export const FleetTable: Component<FleetTableProps> = (props) => {
  const [sorting, setSorting] = createSignal<SortingState>([]);
  const [rowSelection, setRowSelection] = createSignal<RowSelectionState>({});
  const [globalFilter, setGlobalFilter] = createSignal('');

  const columns: ColumnDef<api.Machine>[] = [
    {
      id: 'select',
      header: ({ table }) => (
        <div class="px-1">
          <input
            type="checkbox"
            class="rounded border-white/10 bg-white/5 text-blue-500 focus:ring-blue-500/20"
            checked={table.getIsAllRowsSelected()}
            indeterminate={table.getIsSomeRowsSelected()}
            onChange={table.getToggleAllRowsSelectedHandler()}
          />
        </div>
      ),
      cell: ({ row }) => (
        <div class="px-1">
          <input
            type="checkbox"
            class="rounded border-white/10 bg-white/5 text-blue-500 focus:ring-blue-500/20"
            checked={row.getIsSelected()}
            disabled={!row.getCanSelect()}
            onChange={row.getToggleSelectedHandler()}
          />
        </div>
      ),
    },
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
        const diffMs = new Date().getTime() - lastSeen.getTime();
        const isOnline = diffMs < 300000; // 5 mins
        
        // Map status based on activity and potentially other flags
        // For now using is_active from original data if available
        let status = isOnline ? 'Active' : 'Offline';
        if (info.row.original.is_active === 0) status = 'Compromised'; // Example mapping

        return (
          <div class="flex flex-col gap-1">
            <StatusBadge status={status} pulse={isOnline} />
            <span class="text-[10px] text-slate-500 ml-1">
              {api.formatRelativeTime(info.getValue<string>())}
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
          onClick={() => props.onRevoke([info.row.original.machine_id])}
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
      get rowSelection() { return rowSelection() },
      get globalFilter() { return globalFilter() },
    },
    onSortingChange: setSorting,
    onRowSelectionChange: setRowSelection,
    onGlobalFilterChange: setGlobalFilter,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
  });

  const selectedCount = () => Object.keys(rowSelection()).length;

  const handleBulkRevoke = () => {
    const selectedIds = table.getSelectedRowModel().rows.map(row => row.original.machine_id);
    props.onRevoke(selectedIds);
    setRowSelection({});
  };

  return (
    <div class="space-y-4">
      <div class="flex items-center justify-between gap-4">
        <div class="relative flex-1 max-w-sm">
          <Search class="absolute left-3 top-1/2 -translate-y-1/2 text-slate-500" size={18} />
          <input
            type="text"
            value={globalFilter()}
            onInput={(e) => setGlobalFilter(e.currentTarget.value)}
            placeholder="Search machines..."
            class="w-full pl-10 pr-4 py-2 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500/20 transition-all"
          />
          <Show when={globalFilter()}>
            <button 
              onClick={() => setGlobalFilter('')}
              class="absolute right-3 top-1/2 -translate-y-1/2 text-slate-500 hover:text-white"
            >
              <X size={14} />
            </button>
          </Show>
        </div>

        <Show when={selectedCount() > 0}>
          <button
            onClick={handleBulkRevoke}
            class="flex items-center gap-2 px-4 py-2 bg-rose-500/10 border border-rose-500/20 text-rose-400 rounded-xl text-sm font-bold hover:bg-rose-500/20 transition-all animate-in fade-in slide-in-from-right-4"
          >
            <Trash2 size={16} />
            Revoke Selected ({selectedCount()})
          </button>
        </Show>
      </div>

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
                <tr 
                  class={`border-b border-white/5 hover:bg-white/[0.01] transition-colors group ${row.getIsSelected() ? 'bg-blue-500/[0.03]' : ''}`}
                >
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
