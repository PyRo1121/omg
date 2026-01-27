import { Component, For, Show, createMemo, createSignal, ParentComponent } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import {
  MessageSquare,
  Phone,
  Mail,
  Calendar,
  HelpCircle,
  DollarSign,
  CheckCircle,
  Clock,
  User,
  Tag,
  X,
  Plus,
  MoreHorizontal,
  ChevronRight,
} from 'lucide-solid';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export type NoteType = 'general' | 'call' | 'email' | 'meeting' | 'support' | 'sales' | 'success';
export type TaskType = 'followup' | 'onboarding' | 'renewal' | 'upsell' | 'support';
export type CommunicationType = 'email' | 'chat' | 'phone' | 'meeting' | 'support_ticket';

interface Note {
  id: string;
  type: NoteType;
  content: string;
  createdAt: string;
  createdBy?: string;
}

interface Task {
  id: string;
  type: TaskType;
  title: string;
  dueDate: string;
  completed: boolean;
  priority?: 'low' | 'medium' | 'high';
}

interface Communication {
  id: string;
  type: CommunicationType;
  subject?: string;
  summary: string;
  date: string;
  direction: 'inbound' | 'outbound';
}

interface CRMTag {
  id: string;
  name: string;
  color: string;
}

const noteTypeConfig: Record<NoteType, { icon: typeof MessageSquare; color: string; bg: string; label: string }> = {
  general: { icon: MessageSquare, color: 'text-nebula-400', bg: 'bg-nebula-500/10', label: 'Note' },
  call: { icon: Phone, color: 'text-plasma-400', bg: 'bg-plasma-500/10', label: 'Call' },
  email: { icon: Mail, color: 'text-indigo-400', bg: 'bg-indigo-500/10', label: 'Email' },
  meeting: { icon: Calendar, color: 'text-photon-400', bg: 'bg-photon-500/10', label: 'Meeting' },
  support: { icon: HelpCircle, color: 'text-solar-400', bg: 'bg-solar-500/10', label: 'Support' },
  sales: { icon: DollarSign, color: 'text-aurora-400', bg: 'bg-aurora-500/10', label: 'Sales' },
  success: { icon: CheckCircle, color: 'text-electric-400', bg: 'bg-electric-500/10', label: 'Success' },
};

const taskTypeConfig: Record<TaskType, { icon: typeof Clock; color: string; bg: string; label: string }> = {
  followup: { icon: Phone, color: 'text-plasma-400', bg: 'bg-plasma-500/10', label: 'Follow-up' },
  onboarding: { icon: User, color: 'text-photon-400', bg: 'bg-photon-500/10', label: 'Onboarding' },
  renewal: { icon: Calendar, color: 'text-solar-400', bg: 'bg-solar-500/10', label: 'Renewal' },
  upsell: { icon: DollarSign, color: 'text-aurora-400', bg: 'bg-aurora-500/10', label: 'Upsell' },
  support: { icon: HelpCircle, color: 'text-flare-400', bg: 'bg-flare-500/10', label: 'Support' },
};

const commTypeConfig: Record<CommunicationType, { icon: typeof Mail; color: string; label: string }> = {
  email: { icon: Mail, color: 'text-indigo-400', label: 'Email' },
  chat: { icon: MessageSquare, color: 'text-electric-400', label: 'Chat' },
  phone: { icon: Phone, color: 'text-aurora-400', label: 'Phone' },
  meeting: { icon: Calendar, color: 'text-photon-400', label: 'Meeting' },
  support_ticket: { icon: HelpCircle, color: 'text-solar-400', label: 'Ticket' },
};

interface NoteCardProps {
  note: Note;
  onDelete?: (id: string) => void;
  class?: string;
}

export const NoteCard: Component<NoteCardProps> = (props) => {
  const config = createMemo(() => noteTypeConfig[props.note.type]);
  const IconComponent = config().icon;

  return (
    <div class={cn(
      'rounded-xl border border-white/5 bg-void-850 p-4 transition-all hover:border-white/10',
      props.class
    )}>
      <div class="flex items-start gap-3">
        <div class={cn('p-2 rounded-lg shrink-0', config().bg)}>
          <IconComponent size={14} class={config().color} />
        </div>
        <div class="flex-1 min-w-0">
          <div class="flex items-center justify-between gap-2 mb-2">
            <span class={cn('text-xs font-bold uppercase tracking-wider', config().color)}>
              {config().label}
            </span>
            <div class="flex items-center gap-2">
              <span class="text-2xs text-nebula-600">{props.note.createdAt}</span>
              <Show when={props.onDelete}>
                <button
                  onClick={() => props.onDelete?.(props.note.id)}
                  class="p-1 rounded text-nebula-600 hover:text-flare-400 hover:bg-flare-500/10 transition-colors"
                >
                  <X size={12} />
                </button>
              </Show>
            </div>
          </div>
          <p class="text-sm text-nebula-300 leading-relaxed">{props.note.content}</p>
          <Show when={props.note.createdBy}>
            <p class="text-2xs text-nebula-600 mt-2">by {props.note.createdBy}</p>
          </Show>
        </div>
      </div>
    </div>
  );
};

interface NotesListProps {
  notes: Note[];
  onAddNote?: (content: string, type: NoteType) => void;
  onDeleteNote?: (id: string) => void;
  class?: string;
}

export const NotesList: Component<NotesListProps> = (props) => {
  const [showAdd, setShowAdd] = createSignal(false);
  const [newNoteContent, setNewNoteContent] = createSignal('');
  const [newNoteType, setNewNoteType] = createSignal<NoteType>('general');

  const handleAdd = () => {
    if (newNoteContent().trim() && props.onAddNote) {
      props.onAddNote(newNoteContent(), newNoteType());
      setNewNoteContent('');
      setShowAdd(false);
    }
  };

  return (
    <div class={cn('space-y-4', props.class)}>
      <div class="flex items-center justify-between">
        <h4 class="text-sm font-bold text-white uppercase tracking-wider">Notes</h4>
        <button
          onClick={() => setShowAdd(!showAdd())}
          class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-bold text-indigo-400 bg-indigo-500/10 hover:bg-indigo-500/20 transition-colors"
        >
          <Plus size={12} />
          Add Note
        </button>
      </div>

      <Show when={showAdd()}>
        <div class="rounded-xl border border-indigo-500/30 bg-indigo-500/5 p-4 space-y-3">
          <div class="flex gap-2">
            <For each={Object.entries(noteTypeConfig)}>
              {([type, config]) => {
                const IconComponent = config.icon;
                return (
                  <button
                    onClick={() => setNewNoteType(type as NoteType)}
                    class={cn(
                      'p-2 rounded-lg transition-colors',
                      newNoteType() === type
                        ? cn(config.bg, config.color)
                        : 'text-nebula-500 hover:bg-white/5'
                    )}
                    title={config.label}
                  >
                    <IconComponent size={14} />
                  </button>
                );
              }}
            </For>
          </div>
          <textarea
            value={newNoteContent()}
            onInput={(e) => setNewNoteContent(e.currentTarget.value)}
            placeholder="Write your note..."
            class="w-full h-20 px-3 py-2 rounded-lg bg-void-900 border border-white/10 text-sm text-white placeholder-nebula-600 focus:outline-none focus:border-indigo-500/50 resize-none"
          />
          <div class="flex justify-end gap-2">
            <button
              onClick={() => setShowAdd(false)}
              class="px-3 py-1.5 text-xs font-bold text-nebula-400 hover:text-white transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleAdd}
              class="px-4 py-1.5 rounded-lg text-xs font-bold text-white bg-indigo-600 hover:bg-indigo-500 transition-colors"
            >
              Save Note
            </button>
          </div>
        </div>
      </Show>

      <div class="space-y-2">
        <For each={props.notes}>
          {(note) => (
            <NoteCard note={note} onDelete={props.onDeleteNote} />
          )}
        </For>
        <Show when={props.notes.length === 0}>
          <p class="text-center text-sm text-nebula-600 py-8">No notes yet</p>
        </Show>
      </div>
    </div>
  );
};

interface TagBadgeProps {
  tag: CRMTag;
  onRemove?: () => void;
  class?: string;
}

export const TagBadge: Component<TagBadgeProps> = (props) => {
  return (
    <div
      class={cn(
        'inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-bold transition-colors',
        props.class
      )}
      style={{
        'background-color': `${props.tag.color}20`,
        color: props.tag.color,
        'border': `1px solid ${props.tag.color}40`,
      }}
    >
      <Tag size={10} />
      <span>{props.tag.name}</span>
      <Show when={props.onRemove}>
        <button
          onClick={props.onRemove}
          class="ml-0.5 p-0.5 rounded hover:bg-white/20 transition-colors"
        >
          <X size={10} />
        </button>
      </Show>
    </div>
  );
};

interface TagsManagerProps {
  assignedTags: CRMTag[];
  availableTags: CRMTag[];
  onAssign?: (tagId: string) => void;
  onRemove?: (tagId: string) => void;
  onCreate?: (name: string, color: string) => void;
  class?: string;
}

const presetColors = [
  '#ef4444', '#f97316', '#f59e0b', '#84cc16', '#22c55e',
  '#14b8a6', '#06b6d4', '#3b82f6', '#6366f1', '#8b5cf6',
  '#a855f7', '#d946ef', '#ec4899', '#f43f5e',
];

export const TagsManager: Component<TagsManagerProps> = (props) => {
  const [showAdd, setShowAdd] = createSignal(false);
  const [newTagName, setNewTagName] = createSignal('');
  const [newTagColor, setNewTagColor] = createSignal(presetColors[0]);

  const unassignedTags = createMemo(() =>
    props.availableTags.filter(t => !props.assignedTags.some(at => at.id === t.id))
  );

  const handleCreate = () => {
    if (newTagName().trim() && props.onCreate) {
      props.onCreate(newTagName(), newTagColor());
      setNewTagName('');
      setShowAdd(false);
    }
  };

  return (
    <div class={cn('space-y-4', props.class)}>
      <div class="flex items-center justify-between">
        <h4 class="text-sm font-bold text-white uppercase tracking-wider">Tags</h4>
        <button
          onClick={() => setShowAdd(!showAdd())}
          class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-bold text-photon-400 bg-photon-500/10 hover:bg-photon-500/20 transition-colors"
        >
          <Plus size={12} />
          New Tag
        </button>
      </div>

      <div class="flex flex-wrap gap-2">
        <For each={props.assignedTags}>
          {(tag) => (
            <TagBadge tag={tag} onRemove={() => props.onRemove?.(tag.id)} />
          )}
        </For>
        <Show when={props.assignedTags.length === 0}>
          <p class="text-sm text-nebula-600">No tags assigned</p>
        </Show>
      </div>

      <Show when={unassignedTags().length > 0}>
        <div class="pt-2 border-t border-white/5">
          <p class="text-xs text-nebula-600 mb-2">Available tags:</p>
          <div class="flex flex-wrap gap-2">
            <For each={unassignedTags()}>
              {(tag) => (
                <button
                  onClick={() => props.onAssign?.(tag.id)}
                  class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-bold border border-dashed transition-all hover:scale-105"
                  style={{
                    'border-color': `${tag.color}40`,
                    color: tag.color,
                  }}
                >
                  <Plus size={10} />
                  {tag.name}
                </button>
              )}
            </For>
          </div>
        </div>
      </Show>

      <Show when={showAdd()}>
        <div class="rounded-xl border border-photon-500/30 bg-photon-500/5 p-4 space-y-3">
          <input
            type="text"
            value={newTagName()}
            onInput={(e) => setNewTagName(e.currentTarget.value)}
            placeholder="Tag name"
            class="w-full px-3 py-2 rounded-lg bg-void-900 border border-white/10 text-sm text-white placeholder-nebula-600 focus:outline-none focus:border-photon-500/50"
          />
          <div class="flex flex-wrap gap-2">
            <For each={presetColors}>
              {(color) => (
                <button
                  onClick={() => setNewTagColor(color)}
                  class={cn(
                    'w-6 h-6 rounded-full transition-transform hover:scale-110',
                    newTagColor() === color && 'ring-2 ring-white ring-offset-2 ring-offset-void-900'
                  )}
                  style={{ 'background-color': color }}
                />
              )}
            </For>
          </div>
          <div class="flex justify-end gap-2">
            <button
              onClick={() => setShowAdd(false)}
              class="px-3 py-1.5 text-xs font-bold text-nebula-400 hover:text-white transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleCreate}
              class="px-4 py-1.5 rounded-lg text-xs font-bold text-white bg-photon-600 hover:bg-photon-500 transition-colors"
            >
              Create Tag
            </button>
          </div>
        </div>
      </Show>
    </div>
  );
};

interface TaskCardProps {
  task: Task;
  onToggle?: (id: string) => void;
  class?: string;
}

export const TaskCard: Component<TaskCardProps> = (props) => {
  const config = createMemo(() => taskTypeConfig[props.task.type]);
  const IconComponent = config().icon;

  const priorityColors = {
    low: 'text-nebula-500',
    medium: 'text-solar-400',
    high: 'text-flare-400',
  };

  return (
    <div
      class={cn(
        'rounded-xl border bg-void-850 p-4 transition-all',
        props.task.completed
          ? 'border-aurora-500/20 bg-aurora-500/5'
          : 'border-white/5 hover:border-white/10',
        props.class
      )}
    >
      <div class="flex items-start gap-3">
        <button
          onClick={() => props.onToggle?.(props.task.id)}
          class={cn(
            'mt-0.5 shrink-0 h-5 w-5 rounded-full border-2 flex items-center justify-center transition-colors',
            props.task.completed
              ? 'bg-aurora-500 border-aurora-500 text-white'
              : 'border-nebula-600 hover:border-aurora-500'
          )}
        >
          <Show when={props.task.completed}>
            <CheckCircle size={12} />
          </Show>
        </button>
        <div class="flex-1 min-w-0">
          <p class={cn(
            'text-sm font-medium',
            props.task.completed ? 'text-nebula-500 line-through' : 'text-white'
          )}>
            {props.task.title}
          </p>
          <div class="flex items-center gap-3 mt-2">
            <div class={cn('flex items-center gap-1', config().color)}>
              <IconComponent size={12} />
              <span class="text-2xs font-bold uppercase">{config().label}</span>
            </div>
            <span class="text-2xs text-nebula-600 flex items-center gap-1">
              <Clock size={10} />
              {props.task.dueDate}
            </span>
            <Show when={props.task.priority}>
              <span class={cn('text-2xs font-bold uppercase', priorityColors[props.task.priority!])}>
                {props.task.priority}
              </span>
            </Show>
          </div>
        </div>
      </div>
    </div>
  );
};

interface CommunicationTimelineProps {
  communications: Communication[];
  class?: string;
}

export const CommunicationTimeline: Component<CommunicationTimelineProps> = (props) => {
  return (
    <div class={cn('space-y-4', props.class)}>
      <h4 class="text-sm font-bold text-white uppercase tracking-wider">Communication History</h4>
      <div class="space-y-3">
        <For each={props.communications}>
          {(comm, index) => {
            const config = commTypeConfig[comm.type];
            const IconComponent = config.icon;

            return (
              <div class="flex gap-3">
                <div class="relative flex flex-col items-center">
                  <div class={cn(
                    'w-8 h-8 rounded-full flex items-center justify-center',
                    comm.direction === 'inbound' ? 'bg-indigo-500/10' : 'bg-aurora-500/10'
                  )}>
                    <IconComponent size={14} class={config.color} />
                  </div>
                  <Show when={index() < props.communications.length - 1}>
                    <div class="flex-1 w-px bg-void-700 my-1" />
                  </Show>
                </div>
                <div class="flex-1 pb-4">
                  <div class="flex items-center gap-2 mb-1">
                    <span class="text-sm font-bold text-white">{config.label}</span>
                    <span class={cn(
                      'text-2xs px-1.5 py-0.5 rounded font-bold uppercase',
                      comm.direction === 'inbound'
                        ? 'bg-indigo-500/10 text-indigo-400'
                        : 'bg-aurora-500/10 text-aurora-400'
                    )}>
                      {comm.direction}
                    </span>
                    <span class="text-2xs text-nebula-600">{comm.date}</span>
                  </div>
                  <Show when={comm.subject}>
                    <p class="text-sm font-medium text-nebula-300">{comm.subject}</p>
                  </Show>
                  <p class="text-sm text-nebula-500">{comm.summary}</p>
                </div>
              </div>
            );
          }}
        </For>
        <Show when={props.communications.length === 0}>
          <p class="text-center text-sm text-nebula-600 py-8">No communication history</p>
        </Show>
      </div>
    </div>
  );
};

interface CustomerCardProps {
  name: string;
  email: string;
  company?: string;
  tier: string;
  healthScore: number;
  stage: string;
  tags?: CRMTag[];
  lastActive?: string;
  onClick?: () => void;
  class?: string;
}

export const CustomerCard: Component<CustomerCardProps> = (props) => {
  const tierColors: Record<string, string> = {
    enterprise: 'text-solar-400 bg-solar-500/10 border-solar-500/25',
    team: 'text-electric-400 bg-electric-500/10 border-electric-500/25',
    pro: 'text-indigo-400 bg-indigo-500/10 border-indigo-500/25',
    free: 'text-nebula-400 bg-nebula-500/10 border-nebula-500/25',
  };

  const healthColor = () => {
    if (props.healthScore >= 80) return 'text-aurora-400';
    if (props.healthScore >= 60) return 'text-electric-400';
    if (props.healthScore >= 40) return 'text-solar-400';
    return 'text-flare-400';
  };

  return (
    <button
      onClick={props.onClick}
      class={cn(
        'w-full rounded-2xl border border-white/5 bg-void-850 p-5 text-left transition-all',
        'hover:border-white/10 hover:bg-void-800 hover:shadow-lg',
        'group',
        props.class
      )}
    >
      <div class="flex items-start justify-between mb-3">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-gradient-to-br from-indigo-500 to-photon-600 flex items-center justify-center text-white font-black text-sm">
            {props.name.charAt(0).toUpperCase()}
          </div>
          <div>
            <p class="text-sm font-bold text-white">{props.name}</p>
            <p class="text-xs text-nebula-500">{props.email}</p>
          </div>
        </div>
        <ChevronRight size={16} class="text-nebula-600 group-hover:text-white group-hover:translate-x-1 transition-all" />
      </div>

      <div class="flex items-center gap-2 mb-3">
        <span class={cn(
          'px-2 py-0.5 rounded-full text-2xs font-black uppercase border',
          tierColors[props.tier.toLowerCase()] || tierColors.free
        )}>
          {props.tier}
        </span>
        <span class={cn('text-sm font-bold tabular-nums', healthColor())}>
          {props.healthScore}
        </span>
      </div>

      <Show when={props.tags && props.tags.length > 0}>
        <div class="flex flex-wrap gap-1.5 mb-3">
          <For each={props.tags!.slice(0, 3)}>
            {(tag) => (
              <span
                class="px-2 py-0.5 rounded-full text-2xs font-medium"
                style={{
                  'background-color': `${tag.color}15`,
                  color: tag.color,
                }}
              >
                {tag.name}
              </span>
            )}
          </For>
          <Show when={props.tags!.length > 3}>
            <span class="px-2 py-0.5 rounded-full text-2xs font-medium bg-void-700 text-nebula-500">
              +{props.tags!.length - 3}
            </span>
          </Show>
        </div>
      </Show>

      <Show when={props.lastActive}>
        <p class="text-2xs text-nebula-600">Last active {props.lastActive}</p>
      </Show>
    </button>
  );
};

export default {
  NoteCard,
  NotesList,
  TagBadge,
  TagsManager,
  TaskCard,
  CommunicationTimeline,
  CustomerCard,
};
