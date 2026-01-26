import { Component, For, Show, createSignal } from 'solid-js';
import { createMutation, createQuery, useQueryClient } from '@tanstack/solid-query';
import { Plus, X, Check, Edit } from '../../ui/Icons';
import { apiRequest } from '../../../lib/api';

interface Tag {
  id: string;
  name: string;
  color: string;
  description?: string;
}

interface TagsManagerProps {
  customerId: string;
  assignedTags: Tag[];
  onRefresh?: () => void;
}

const DEFAULT_COLORS = [
  '#ef4444',
  '#f59e0b',
  '#10b981',
  '#06b6d4',
  '#6366f1',
  '#8b5cf6',
  '#ec4899',
  '#64748b',
];

export const TagsManager: Component<TagsManagerProps> = props => {
  const queryClient = useQueryClient();
  const [isAdding, setIsAdding] = createSignal(false);
  const [newTagName, setNewTagName] = createSignal('');
  const [newTagColor, setNewTagColor] = createSignal(DEFAULT_COLORS[4]);

  const allTagsQuery = createQuery(() => ({
    queryKey: ['all-tags'],
    queryFn: () => apiRequest<{ tags: Tag[] }>('/api/admin/tags'),
  }));

  const allTags = () => allTagsQuery.data?.tags || [];
  const assignedTagIds = () => new Set(props.assignedTags.map(t => t.id));
  const availableTags = () => allTags().filter(t => !assignedTagIds().has(t.id));

  const createTagMutation = createMutation(() => ({
    mutationFn: async (data: { name: string; color: string }) => {
      return apiRequest<{ success: boolean; tag: Tag }>('/api/admin/tags', {
        method: 'POST',
        body: JSON.stringify(data),
      });
    },
    onSuccess: data => {
      setIsAdding(false);
      setNewTagName('');
      queryClient.invalidateQueries({ queryKey: ['all-tags'] });
      if (data.tag) {
        assignTagMutation.mutate(data.tag.id);
      }
    },
  }));

  const assignTagMutation = createMutation(() => ({
    mutationFn: async (tagId: string) => {
      return apiRequest<{ success: boolean }>(
        `/api/admin/customers/${props.customerId}/tags/${tagId}`,
        {
          method: 'POST',
        }
      );
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-user-detail', props.customerId] });
      props.onRefresh?.();
    },
  }));

  const removeTagMutation = createMutation(() => ({
    mutationFn: async (tagId: string) => {
      return apiRequest<{ success: boolean }>(
        `/api/admin/customers/${props.customerId}/tags/${tagId}`,
        {
          method: 'DELETE',
        }
      );
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-user-detail', props.customerId] });
      props.onRefresh?.();
    },
  }));

  const handleCreateTag = () => {
    if (!newTagName().trim()) return;
    createTagMutation.mutate({
      name: newTagName().trim(),
      color: newTagColor(),
    });
  };

  return (
    <div class="space-y-4">
      <div class="flex items-center justify-between">
        <h4 class="text-sm font-bold text-white">Tags</h4>
        <button
          onClick={() => setIsAdding(!isAdding())}
          class="flex items-center gap-1.5 rounded-xl bg-indigo-500/10 px-3 py-1.5 text-xs font-bold text-indigo-400 transition-all hover:bg-indigo-500/20"
        >
          <Plus size={14} />
          Create Tag
        </button>
      </div>

      <Show when={isAdding()}>
        <div class="animate-in fade-in slide-in-from-top-2 space-y-3 rounded-2xl border border-indigo-500/20 bg-indigo-500/5 p-4 duration-200">
          <div class="flex items-center gap-3">
            <input
              type="text"
              value={newTagName()}
              onInput={e => setNewTagName(e.currentTarget.value)}
              placeholder="Tag name..."
              class="flex-1 rounded-xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white placeholder-slate-500 focus:ring-2 focus:ring-indigo-500/20 focus:outline-none"
            />
          </div>
          <div class="flex items-center gap-2">
            <span class="text-xs text-slate-500">Color:</span>
            <div class="flex items-center gap-1">
              <For each={DEFAULT_COLORS}>
                {color => (
                  <button
                    onClick={() => setNewTagColor(color)}
                    class={`h-6 w-6 rounded-lg transition-all ${
                      newTagColor() === color
                        ? 'ring-2 ring-white ring-offset-2 ring-offset-black'
                        : ''
                    }`}
                    style={{ 'background-color': color }}
                  />
                )}
              </For>
            </div>
          </div>
          <div class="flex items-center justify-end gap-2">
            <button
              onClick={() => setIsAdding(false)}
              class="px-4 py-2 text-sm font-bold text-slate-400 transition-colors hover:text-white"
            >
              Cancel
            </button>
            <button
              onClick={handleCreateTag}
              disabled={!newTagName().trim() || createTagMutation.isPending}
              class="rounded-xl bg-indigo-500 px-4 py-2 text-sm font-bold text-white transition-all hover:bg-indigo-600 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {createTagMutation.isPending ? 'Creating...' : 'Create & Assign'}
            </button>
          </div>
        </div>
      </Show>

      <div class="flex flex-wrap gap-2">
        <For each={props.assignedTags}>
          {tag => (
            <div
              class="group flex items-center gap-2 rounded-xl px-3 py-1.5 text-xs font-bold transition-all"
              style={{
                'background-color': `${tag.color}20`,
                color: tag.color,
                border: `1px solid ${tag.color}40`,
              }}
            >
              <span>{tag.name}</span>
              <button
                onClick={() => removeTagMutation.mutate(tag.id)}
                class="opacity-50 transition-opacity hover:opacity-100"
                disabled={removeTagMutation.isPending}
              >
                <X size={12} />
              </button>
            </div>
          )}
        </For>
        <Show when={props.assignedTags.length === 0}>
          <span class="text-xs text-slate-500">No tags assigned</span>
        </Show>
      </div>

      <Show when={availableTags().length > 0}>
        <div class="border-t border-white/5 pt-3">
          <p class="mb-2 text-[10px] font-bold tracking-widest text-slate-500 uppercase">
            Quick Add
          </p>
          <div class="flex flex-wrap gap-2">
            <For each={availableTags().slice(0, 6)}>
              {tag => (
                <button
                  onClick={() => assignTagMutation.mutate(tag.id)}
                  disabled={assignTagMutation.isPending}
                  class="flex items-center gap-1.5 rounded-xl border border-white/10 bg-white/[0.02] px-3 py-1.5 text-xs font-bold text-slate-400 transition-all hover:border-white/20 hover:text-white disabled:opacity-50"
                >
                  <div class="h-2 w-2 rounded-full" style={{ 'background-color': tag.color }} />
                  {tag.name}
                  <Plus size={10} />
                </button>
              )}
            </For>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default TagsManager;
