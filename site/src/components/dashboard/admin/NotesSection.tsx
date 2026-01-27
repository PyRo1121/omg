import { Component, createSignal, For, Show } from 'solid-js';
import { MessageSquare, Pin, Trash2, Plus, X } from 'lucide-solid';
import { formatRelativeTime } from '../../../lib/api';

interface Note {
  id: string;
  customer_id: string;
  content: string;
  note_type: string;
  is_pinned: number;
  author_email?: string;
  created_at: string;
  updated_at: string;
}

interface NotesSectionProps {
  customerId: string;
  notes: Note[];
  onAddNote: (content: string, noteType: string) => void;
  onDeleteNote: (noteId: string) => void;
  onPinNote?: (noteId: string, isPinned: boolean) => void;
  isLoading?: boolean;
}

const NOTE_TYPES = [
  { value: 'general', label: 'General', color: 'text-slate-400' },
  { value: 'call', label: 'Call', color: 'text-blue-400' },
  { value: 'email', label: 'Email', color: 'text-cyan-400' },
  { value: 'meeting', label: 'Meeting', color: 'text-purple-400' },
  { value: 'support', label: 'Support', color: 'text-amber-400' },
  { value: 'sales', label: 'Sales', color: 'text-emerald-400' },
  { value: 'success', label: 'Success', color: 'text-pink-400' },
];

export const NotesSection: Component<NotesSectionProps> = (props) => {
  const [newNote, setNewNote] = createSignal('');
  const [noteType, setNoteType] = createSignal('general');
  const [isAdding, setIsAdding] = createSignal(false);

  const handleAdd = () => {
    const content = newNote().trim();
    if (content) {
      props.onAddNote(content, noteType());
      setNewNote('');
      setNoteType('general');
      setIsAdding(false);
    }
  };

  const sortedNotes = () =>
    [...props.notes].sort((a, b) => {
      // Pinned notes first
      if (a.is_pinned && !b.is_pinned) return -1;
      if (!a.is_pinned && b.is_pinned) return 1;
      // Then by date (newest first)
      return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
    });

  const getNoteTypeColor = (type: string) => {
    return NOTE_TYPES.find((t) => t.value === type)?.color || 'text-slate-400';
  };

  return (
    <div class="rounded-xl border border-white/10 bg-white/5 p-6">
      <div class="mb-4 flex items-center justify-between">
        <h4 class="flex items-center gap-2 text-lg font-bold text-white">
          <MessageSquare size={18} />
          Customer Notes
        </h4>
        <button
          onClick={() => setIsAdding(!isAdding())}
          class="flex items-center gap-1.5 rounded-lg bg-indigo-500/20 px-3 py-1.5 text-sm font-bold text-indigo-400 transition-colors hover:bg-indigo-500/30"
        >
          {isAdding() ? (
            <>
              <X size={14} />
              Cancel
            </>
          ) : (
            <>
              <Plus size={14} />
              Add Note
            </>
          )}
        </button>
      </div>

      <Show when={isAdding()}>
        <div class="mb-4 rounded-lg border border-white/10 bg-white/5 p-4">
          <div class="mb-3">
            <label class="mb-2 block text-xs font-bold uppercase tracking-wider text-slate-400">
              Note Type
            </label>
            <div class="flex flex-wrap gap-2">
              <For each={NOTE_TYPES}>
                {(type) => (
                  <button
                    onClick={() => setNoteType(type.value)}
                    class={`rounded-lg px-3 py-1.5 text-xs font-bold transition-all ${
                      noteType() === type.value
                        ? 'bg-white text-black'
                        : 'bg-white/5 text-slate-400 hover:bg-white/10 hover:text-white'
                    }`}
                  >
                    {type.label}
                  </button>
                )}
              </For>
            </div>
          </div>

          <textarea
            value={newNote()}
            onInput={(e) => setNewNote(e.currentTarget.value)}
            placeholder="Add a note about this customer..."
            class="w-full rounded-lg border border-white/10 bg-white/5 p-3 text-sm text-white placeholder-slate-500 focus:border-indigo-500/50 focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
            rows={4}
          />

          <div class="mt-3 flex justify-end gap-2">
            <button
              onClick={() => {
                setIsAdding(false);
                setNewNote('');
                setNoteType('general');
              }}
              class="rounded-lg px-4 py-2 text-sm font-bold text-slate-400 transition-colors hover:bg-white/5 hover:text-white"
            >
              Cancel
            </button>
            <button
              onClick={handleAdd}
              disabled={!newNote().trim()}
              class="rounded-lg bg-indigo-500 px-4 py-2 text-sm font-bold text-white transition-colors hover:bg-indigo-600 disabled:cursor-not-allowed disabled:opacity-50"
            >
              Save Note
            </button>
          </div>
        </div>
      </Show>

      <Show when={props.isLoading}>
        <div class="space-y-3">
          <div class="h-24 animate-pulse rounded-lg bg-white/5" />
          <div class="h-24 animate-pulse rounded-lg bg-white/5" />
        </div>
      </Show>

      <Show when={!props.isLoading}>
        <div class="space-y-3">
          <Show when={props.notes.length === 0 && !isAdding()}>
            <div class="py-12 text-center">
              <MessageSquare size={32} class="mx-auto mb-3 text-slate-600" />
              <p class="text-sm font-medium text-slate-500">No notes yet</p>
              <p class="mt-1 text-xs text-slate-600">Add a note to track customer interactions</p>
            </div>
          </Show>

          <For each={sortedNotes()}>
            {(note) => (
              <div class="group rounded-lg border border-white/5 bg-white/5 p-4 transition-all hover:border-white/10 hover:bg-white/10">
                <div class="mb-3 flex items-start justify-between">
                  <div class="flex items-center gap-2">
                    <Show when={note.is_pinned}>
                      <Pin size={12} class="text-amber-400" fill="currentColor" />
                    </Show>
                    <span class={`text-xs font-bold uppercase ${getNoteTypeColor(note.note_type)}`}>
                      {NOTE_TYPES.find((t) => t.value === note.note_type)?.label || note.note_type}
                    </span>
                    <span class="text-xs text-slate-500">•</span>
                    <span class="text-xs text-slate-400">
                      {formatRelativeTime(note.created_at)}
                      {note.author_email && ` by ${note.author_email}`}
                    </span>
                  </div>
                  <button
                    onClick={() => props.onDeleteNote(note.id)}
                    class="opacity-0 transition-opacity group-hover:opacity-100"
                    title="Delete note"
                  >
                    <Trash2 size={14} class="text-rose-400 transition-colors hover:text-rose-300" />
                  </button>
                </div>
                <p class="whitespace-pre-wrap text-sm leading-relaxed text-white">{note.content}</p>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};
