import { Component, For, Show, createSignal } from 'solid-js';
import { createMutation, useQueryClient } from '@tanstack/solid-query';
import { Plus, Edit, Trash2, Star, Mail, Zap, User, CheckCircle, FileText } from '../../ui/Icons';
import { apiRequest, formatRelativeTime } from '../../../lib/api';

interface Note {
  id: string;
  customer_id: string;
  author_id: string;
  author_email?: string;
  note_type: 'general' | 'call' | 'email' | 'meeting' | 'support' | 'sales' | 'success';
  content: string;
  is_pinned: boolean;
  created_at: string;
  updated_at: string;
}

interface NotesPanelProps {
  customerId: string;
  notes: Note[];
  onRefresh?: () => void;
}

const NOTE_TYPE_ICONS: Record<string, typeof FileText> = {
  general: FileText,
  call: Zap,
  email: Mail,
  meeting: User,
  support: CheckCircle,
  sales: Star,
  success: CheckCircle,
};

const NOTE_TYPE_COLORS: Record<string, string> = {
  general: 'text-slate-400 bg-slate-500/10',
  call: 'text-emerald-400 bg-emerald-500/10',
  email: 'text-indigo-400 bg-indigo-500/10',
  meeting: 'text-violet-400 bg-violet-500/10',
  support: 'text-amber-400 bg-amber-500/10',
  sales: 'text-pink-400 bg-pink-500/10',
  success: 'text-cyan-400 bg-cyan-500/10',
};

export const NotesPanel: Component<NotesPanelProps> = props => {
  const queryClient = useQueryClient();
  const [isAdding, setIsAdding] = createSignal(false);
  const [newNoteContent, setNewNoteContent] = createSignal('');
  const [newNoteType, setNewNoteType] = createSignal<Note['note_type']>('general');
  const [editingNote, setEditingNote] = createSignal<string | null>(null);
  const [editContent, setEditContent] = createSignal('');

  const createNoteMutation = createMutation(() => ({
    mutationFn: async (data: { content: string; note_type: string }) => {
      return apiRequest<{ success: boolean; note: Note }>(
        `/api/admin/customers/${props.customerId}/notes`,
        {
          method: 'POST',
          body: JSON.stringify(data),
        }
      );
    },
    onSuccess: () => {
      setIsAdding(false);
      setNewNoteContent('');
      setNewNoteType('general');
      queryClient.invalidateQueries({ queryKey: ['admin-user-detail', props.customerId] });
      props.onRefresh?.();
    },
  }));

  const updateNoteMutation = createMutation(() => ({
    mutationFn: async ({ noteId, content }: { noteId: string; content: string }) => {
      return apiRequest<{ success: boolean }>(`/api/admin/notes/${noteId}`, {
        method: 'PUT',
        body: JSON.stringify({ content }),
      });
    },
    onSuccess: () => {
      setEditingNote(null);
      setEditContent('');
      queryClient.invalidateQueries({ queryKey: ['admin-user-detail', props.customerId] });
      props.onRefresh?.();
    },
  }));

  const deleteNoteMutation = createMutation(() => ({
    mutationFn: async (noteId: string) => {
      return apiRequest<{ success: boolean }>(`/api/admin/notes/${noteId}`, {
        method: 'DELETE',
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-user-detail', props.customerId] });
      props.onRefresh?.();
    },
  }));

  const togglePinMutation = createMutation(() => ({
    mutationFn: async ({ noteId, isPinned }: { noteId: string; isPinned: boolean }) => {
      return apiRequest<{ success: boolean }>(`/api/admin/notes/${noteId}`, {
        method: 'PUT',
        body: JSON.stringify({ is_pinned: !isPinned }),
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-user-detail', props.customerId] });
      props.onRefresh?.();
    },
  }));

  const handleSubmit = () => {
    if (!newNoteContent().trim()) return;
    createNoteMutation.mutate({
      content: newNoteContent(),
      note_type: newNoteType(),
    });
  };

  const handleUpdate = (noteId: string) => {
    if (!editContent().trim()) return;
    updateNoteMutation.mutate({ noteId, content: editContent() });
  };

  const sortedNotes = () => {
    return [...props.notes].sort((a, b) => {
      if (a.is_pinned && !b.is_pinned) return -1;
      if (!a.is_pinned && b.is_pinned) return 1;
      return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
    });
  };

  return (
    <div class="space-y-4">
      <div class="flex items-center justify-between">
        <h4 class="text-sm font-bold text-white">Notes & Activity</h4>
        <button
          onClick={() => setIsAdding(!isAdding())}
          class="flex items-center gap-1.5 rounded-xl bg-indigo-500/10 px-3 py-1.5 text-xs font-bold text-indigo-400 transition-all hover:bg-indigo-500/20"
        >
          <Plus size={14} />
          Add Note
        </button>
      </div>

      <Show when={isAdding()}>
        <div class="animate-in fade-in slide-in-from-top-2 space-y-3 rounded-2xl border border-indigo-500/20 bg-indigo-500/5 p-4 duration-200">
          <div class="flex items-center gap-2">
            <select
              value={newNoteType()}
              onChange={e => setNewNoteType(e.currentTarget.value as Note['note_type'])}
              class="appearance-none rounded-xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white focus:ring-2 focus:ring-indigo-500/20 focus:outline-none"
            >
              <option value="general">General</option>
              <option value="call">Call</option>
              <option value="email">Email</option>
              <option value="meeting">Meeting</option>
              <option value="support">Support</option>
              <option value="sales">Sales</option>
              <option value="success">Success</option>
            </select>
          </div>
          <textarea
            value={newNoteContent()}
            onInput={e => setNewNoteContent(e.currentTarget.value)}
            placeholder="Write a note about this customer..."
            rows={3}
            class="w-full resize-none rounded-xl border border-white/10 bg-white/5 px-4 py-3 text-sm text-white placeholder-slate-500 focus:ring-2 focus:ring-indigo-500/20 focus:outline-none"
          />
          <div class="flex items-center justify-end gap-2">
            <button
              onClick={() => setIsAdding(false)}
              class="px-4 py-2 text-sm font-bold text-slate-400 transition-colors hover:text-white"
            >
              Cancel
            </button>
            <button
              onClick={handleSubmit}
              disabled={!newNoteContent().trim() || createNoteMutation.isPending}
              class="rounded-xl bg-indigo-500 px-4 py-2 text-sm font-bold text-white transition-all hover:bg-indigo-600 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {createNoteMutation.isPending ? 'Saving...' : 'Save Note'}
            </button>
          </div>
        </div>
      </Show>

      <div class="space-y-3">
        <For each={sortedNotes()}>
          {note => {
            const Icon = NOTE_TYPE_ICONS[note.note_type] || FileText;
            const colorClass = NOTE_TYPE_COLORS[note.note_type] || NOTE_TYPE_COLORS.general;
            const isEditing = editingNote() === note.id;

            return (
              <div
                class={`group rounded-2xl border p-4 transition-all ${
                  note.is_pinned
                    ? 'border-amber-500/20 bg-amber-500/5'
                    : 'border-white/5 bg-white/[0.02] hover:border-white/10'
                }`}
              >
                <div class="flex items-start gap-3">
                  <div class={`shrink-0 rounded-xl p-2 ${colorClass}`}>
                    <Icon size={14} />
                  </div>
                  <div class="min-w-0 flex-1">
                    <div class="flex items-start justify-between gap-2">
                      <div class="flex items-center gap-2">
                        <span class="text-xs font-bold text-white capitalize">
                          {note.note_type}
                        </span>
                        <Show when={note.is_pinned}>
                          <span class="text-amber-400">
                            <Star size={12} class="fill-current" />
                          </span>
                        </Show>
                      </div>
                      <div class="flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100">
                        <button
                          onClick={() =>
                            togglePinMutation.mutate({ noteId: note.id, isPinned: note.is_pinned })
                          }
                          class={`rounded-lg p-1.5 transition-colors ${
                            note.is_pinned
                              ? 'text-amber-400 hover:bg-amber-500/10'
                              : 'text-slate-500 hover:bg-white/5 hover:text-amber-400'
                          }`}
                          title={note.is_pinned ? 'Unpin' : 'Pin'}
                        >
                          <Star size={12} class={note.is_pinned ? 'fill-current' : ''} />
                        </button>
                        <button
                          onClick={() => {
                            setEditingNote(note.id);
                            setEditContent(note.content);
                          }}
                          class="rounded-lg p-1.5 text-slate-500 transition-colors hover:bg-white/5 hover:text-white"
                        >
                          <Edit size={12} />
                        </button>
                        <button
                          onClick={() => {
                            if (confirm('Delete this note?')) {
                              deleteNoteMutation.mutate(note.id);
                            }
                          }}
                          class="rounded-lg p-1.5 text-slate-500 transition-colors hover:bg-rose-500/10 hover:text-rose-400"
                        >
                          <Trash2 size={12} />
                        </button>
                      </div>
                    </div>

                    <Show
                      when={isEditing}
                      fallback={
                        <p class="mt-2 text-sm whitespace-pre-wrap text-slate-300">
                          {note.content}
                        </p>
                      }
                    >
                      <div class="mt-2 space-y-2">
                        <textarea
                          value={editContent()}
                          onInput={e => setEditContent(e.currentTarget.value)}
                          rows={3}
                          class="w-full resize-none rounded-xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white focus:ring-2 focus:ring-indigo-500/20 focus:outline-none"
                        />
                        <div class="flex items-center justify-end gap-2">
                          <button
                            onClick={() => setEditingNote(null)}
                            class="px-3 py-1.5 text-xs font-bold text-slate-400 transition-colors hover:text-white"
                          >
                            Cancel
                          </button>
                          <button
                            onClick={() => handleUpdate(note.id)}
                            disabled={updateNoteMutation.isPending}
                            class="rounded-lg bg-indigo-500 px-3 py-1.5 text-xs font-bold text-white hover:bg-indigo-600 disabled:opacity-50"
                          >
                            Save
                          </button>
                        </div>
                      </div>
                    </Show>

                    <div class="mt-3 flex items-center gap-3 text-[10px] text-slate-500">
                      <span>{note.author_email || 'Admin'}</span>
                      <span>•</span>
                      <span>{formatRelativeTime(note.created_at)}</span>
                    </div>
                  </div>
                </div>
              </div>
            );
          }}
        </For>

        <Show when={props.notes.length === 0}>
          <div class="py-8 text-center text-slate-500">
            <FileText size={32} class="mx-auto mb-2 opacity-50" />
            <p class="text-sm">No notes yet</p>
            <p class="mt-1 text-xs">Add a note to track customer interactions</p>
          </div>
        </Show>
      </div>
    </div>
  );
};

export default NotesPanel;
