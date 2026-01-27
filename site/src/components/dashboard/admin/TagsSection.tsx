import { Component, createSignal, For, Show } from 'solid-js';
import { Tag, X, Plus, Check } from 'lucide-solid';

interface CustomerTag {
  id: string;
  name: string;
  color: string;
  description: string | null;
  usage_count?: number;
}

interface TagsSectionProps {
  customerId: string;
  customerTags: CustomerTag[];
  allTags: CustomerTag[];
  onAssignTag: (tagId: string) => void;
  onRemoveTag: (tagId: string) => void;
  onCreateTag?: (name: string, color: string) => void;
  isLoading?: boolean;
}

const TAG_COLORS = [
  { value: '#10b981', label: 'Emerald' },
  { value: '#06b6d4', label: 'Cyan' },
  { value: '#3b82f6', label: 'Blue' },
  { value: '#8b5cf6', label: 'Purple' },
  { value: '#ec4899', label: 'Pink' },
  { value: '#f59e0b', label: 'Amber' },
  { value: '#ef4444', label: 'Red' },
  { value: '#6b7280', label: 'Gray' },
];

export const TagsSection: Component<TagsSectionProps> = (props) => {
  const [isAdding, setIsAdding] = createSignal(false);
  const [showNewTagForm, setShowNewTagForm] = createSignal(false);
  const [newTagName, setNewTagName] = createSignal('');
  const [newTagColor, setNewTagColor] = createSignal(TAG_COLORS[0].value);

  const availableTags = () => {
    const assignedIds = new Set(props.customerTags.map((t) => t.id));
    return props.allTags.filter((t) => !assignedIds.has(t.id));
  };

  const handleAssignTag = (tagId: string) => {
    props.onAssignTag(tagId);
    setIsAdding(false);
  };

  const handleCreateTag = () => {
    const name = newTagName().trim();
    if (name && props.onCreateTag) {
      props.onCreateTag(name, newTagColor());
      setNewTagName('');
      setNewTagColor(TAG_COLORS[0].value);
      setShowNewTagForm(false);
    }
  };

  return (
    <div class="rounded-xl border border-white/10 bg-white/5 p-6">
      <div class="mb-4 flex items-center justify-between">
        <h4 class="flex items-center gap-2 text-lg font-bold text-white">
          <Tag size={18} />
          Tags
        </h4>
        <button
          onClick={() => {
            setIsAdding(!isAdding());
            setShowNewTagForm(false);
          }}
          class="flex items-center gap-1.5 rounded-lg bg-purple-500/20 px-3 py-1.5 text-sm font-bold text-purple-400 transition-colors hover:bg-purple-500/30"
        >
          {isAdding() ? (
            <>
              <X size={14} />
              Cancel
            </>
          ) : (
            <>
              <Plus size={14} />
              Add Tag
            </>
          )}
        </button>
      </div>

      <Show when={isAdding()}>
        <div class="mb-4 space-y-3">
          <Show when={!showNewTagForm()}>
            <div class="rounded-lg border border-white/10 bg-white/5 p-4">
              <p class="mb-3 text-xs font-bold uppercase tracking-wider text-slate-400">
                Select Existing Tag
              </p>

              <Show when={availableTags().length > 0}>
                <div class="space-y-2">
                  <For each={availableTags()}>
                    {(tag) => (
                      <button
                        onClick={() => handleAssignTag(tag.id)}
                        class="flex w-full items-center justify-between rounded-lg border border-white/5 bg-white/5 p-3 text-left transition-all hover:border-white/10 hover:bg-white/10"
                      >
                        <div class="flex items-center gap-3">
                          <div
                            class="h-3 w-3 shrink-0 rounded-full"
                            style={{ background: tag.color }}
                          />
                          <div class="min-w-0">
                            <p class="text-sm font-medium text-white">{tag.name}</p>
                            <Show when={tag.description}>
                              <p class="text-xs text-slate-400">{tag.description}</p>
                            </Show>
                          </div>
                        </div>
                        <Show when={tag.usage_count}>
                          <span class="text-xs text-slate-500">{tag.usage_count} uses</span>
                        </Show>
                      </button>
                    )}
                  </For>
                </div>
              </Show>

              <Show when={availableTags().length === 0}>
                <p class="py-4 text-center text-sm text-slate-500">All tags assigned</p>
              </Show>

              <Show when={props.onCreateTag}>
                <button
                  onClick={() => setShowNewTagForm(true)}
                  class="mt-3 w-full rounded-lg border border-dashed border-white/20 bg-white/5 py-2.5 text-sm font-bold text-slate-400 transition-colors hover:border-white/30 hover:bg-white/10 hover:text-white"
                >
                  + Create New Tag
                </button>
              </Show>
            </div>
          </Show>

          <Show when={showNewTagForm()}>
            <div class="rounded-lg border border-white/10 bg-white/5 p-4">
              <p class="mb-3 text-xs font-bold uppercase tracking-wider text-slate-400">
                Create New Tag
              </p>

              <div class="mb-3">
                <label class="mb-2 block text-xs font-medium text-slate-400">Tag Name</label>
                <input
                  type="text"
                  value={newTagName()}
                  onInput={(e) => setNewTagName(e.currentTarget.value)}
                  placeholder="e.g., High Priority, Enterprise, Champion"
                  class="w-full rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-sm text-white placeholder-slate-500 focus:border-purple-500/50 focus:outline-none focus:ring-2 focus:ring-purple-500/20"
                />
              </div>

              <div class="mb-3">
                <label class="mb-2 block text-xs font-medium text-slate-400">Color</label>
                <div class="flex flex-wrap gap-2">
                  <For each={TAG_COLORS}>
                    {(color) => (
                      <button
                        onClick={() => setNewTagColor(color.value)}
                        class="group relative h-8 w-8 rounded-lg border-2 transition-all hover:scale-110"
                        style={{
                          background: color.value,
                          'border-color':
                            newTagColor() === color.value ? color.value : 'transparent',
                        }}
                        title={color.label}
                      >
                        <Show when={newTagColor() === color.value}>
                          <Check size={14} class="absolute inset-0 m-auto text-white" />
                        </Show>
                      </button>
                    )}
                  </For>
                </div>
              </div>

              <div class="flex gap-2">
                <button
                  onClick={() => {
                    setShowNewTagForm(false);
                    setNewTagName('');
                  }}
                  class="flex-1 rounded-lg px-4 py-2 text-sm font-bold text-slate-400 transition-colors hover:bg-white/5 hover:text-white"
                >
                  Back
                </button>
                <button
                  onClick={handleCreateTag}
                  disabled={!newTagName().trim()}
                  class="flex-1 rounded-lg bg-purple-500 px-4 py-2 text-sm font-bold text-white transition-colors hover:bg-purple-600 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  Create Tag
                </button>
              </div>
            </div>
          </Show>
        </div>
      </Show>

      <Show when={props.isLoading}>
        <div class="flex flex-wrap gap-2">
          <div class="h-7 w-20 animate-pulse rounded-full bg-white/5" />
          <div class="h-7 w-24 animate-pulse rounded-full bg-white/5" />
          <div class="h-7 w-16 animate-pulse rounded-full bg-white/5" />
        </div>
      </Show>

      <Show when={!props.isLoading}>
        <div class="flex flex-wrap gap-2">
          <Show when={props.customerTags.length === 0 && !isAdding()}>
            <div class="w-full py-8 text-center">
              <Tag size={32} class="mx-auto mb-3 text-slate-600" />
              <p class="text-sm font-medium text-slate-500">No tags assigned</p>
              <p class="mt-1 text-xs text-slate-600">Add tags to categorize this customer</p>
            </div>
          </Show>

          <For each={props.customerTags}>
            {(tag) => (
              <div
                class="group flex items-center gap-2 rounded-full px-3 py-1.5 text-sm font-medium transition-all hover:pr-2"
                style={{
                  background: `${tag.color}20`,
                  color: tag.color,
                  border: `1px solid ${tag.color}30`,
                }}
              >
                <span>{tag.name}</span>
                <button
                  onClick={() => props.onRemoveTag(tag.id)}
                  class="opacity-0 transition-all group-hover:opacity-100"
                  title="Remove tag"
                >
                  <X size={14} class="hover:scale-110" />
                </button>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};
