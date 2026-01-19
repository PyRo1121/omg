import { JSX, For, Show } from 'solid-js';

interface Column<T> {
  key: string;
  header: string;
  render?: (item: T, index: number) => JSX.Element;
  class?: string;
  headerClass?: string;
}

interface TableProps<T> {
  data: T[];
  columns: Column<T>[];
  onRowClick?: (item: T) => void;
  emptyMessage?: string;
  emptyIcon?: string;
  loading?: boolean;
  stickyHeader?: boolean;
}

export function Table<T extends object>(props: TableProps<T>) {
  return (
    <div class="overflow-hidden rounded-2xl border border-slate-800 bg-slate-900/50">
      <div class="overflow-x-auto">
        <table class="w-full">
          <thead class={`bg-slate-800/70 ${props.stickyHeader ? 'sticky top-0 z-10' : ''}`}>
            <tr>
              <For each={props.columns}>
                {col => (
                  <th
                    class={`px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-slate-400 ${col.headerClass || ''}`}
                  >
                    {col.header}
                  </th>
                )}
              </For>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-800/50">
            <Show when={!props.loading && props.data.length > 0}>
              <For each={props.data}>
                {(item, index) => (
                  <tr
                    class={`transition-colors hover:bg-slate-800/30 ${props.onRowClick ? 'cursor-pointer' : ''}`}
                    onClick={() => props.onRowClick?.(item)}
                  >
                    <For each={props.columns}>
                      {col => (
                        <td class={`px-4 py-3 ${col.class || ''}`}>
                          {col.render
                            ? col.render(item, index())
                            : String(item[col.key as keyof T] ?? '')}
                        </td>
                      )}
                    </For>
                  </tr>
                )}
              </For>
            </Show>
          </tbody>
        </table>
      </div>

      <Show when={!props.loading && props.data.length === 0}>
        <div class="flex flex-col items-center justify-center py-16 text-center">
          <div class="mb-4 text-5xl opacity-50">{props.emptyIcon || '📭'}</div>
          <p class="text-lg text-slate-400">{props.emptyMessage || 'No data available'}</p>
        </div>
      </Show>

      <Show when={props.loading}>
        <div class="flex items-center justify-center py-16">
          <div class="h-8 w-8 animate-spin rounded-full border-2 border-indigo-500 border-t-transparent" />
        </div>
      </Show>
    </div>
  );
}

export default Table;
