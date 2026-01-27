/**
 * SegmentBuilder - Visual Segment Builder for Customer Filtering
 * 
 * Enterprise-grade component for building complex customer segments
 * with support for multiple fields, operators, and nested AND/OR logic.
 */

import { Component, For, Show, createSignal, createMemo, createEffect } from 'solid-js';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import { 
  Plus, 
  Trash2, 
  GripVertical, 
  Save, 
  X, 
  Users,
  ChevronDown,
  Filter,
  Layers
} from 'lucide-solid';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/** Available field types for segment filtering */
export type SegmentFieldType = 
  | 'tier' 
  | 'lifecycle_stage' 
  | 'engagement_score' 
  | 'last_active' 
  | 'total_commands' 
  | 'machine_count' 
  | 'created_at';

/** Available comparison operators */
export type SegmentOperator = 
  | 'equals' 
  | 'not_equals' 
  | 'greater_than' 
  | 'less_than' 
  | 'contains' 
  | 'between';

/** Logical operators for combining conditions */
export type LogicalOperator = 'AND' | 'OR';

/** A single filter condition */
export interface SegmentCondition {
  id: string;
  field: SegmentFieldType;
  operator: SegmentOperator;
  value: string | number | [number, number];
}

/** A group of conditions with a logical operator */
export interface SegmentGroup {
  id: string;
  logic: LogicalOperator;
  conditions: SegmentCondition[];
}

/** Complete segment definition */
export interface Segment {
  id: string;
  name: string;
  description: string;
  groups: SegmentGroup[];
  rootLogic: LogicalOperator;
}

/** Field metadata for UI rendering */
interface FieldMeta {
  label: string;
  icon: typeof Users;
  type: 'string' | 'number' | 'date' | 'enum';
  options?: { value: string; label: string }[];
}

const FIELD_METADATA: Record<SegmentFieldType, FieldMeta> = {
  tier: {
    label: 'Tier',
    icon: Layers,
    type: 'enum',
    options: [
      { value: 'free', label: 'Free' },
      { value: 'pro', label: 'Pro' },
      { value: 'team', label: 'Team' },
      { value: 'enterprise', label: 'Enterprise' },
    ],
  },
  lifecycle_stage: {
    label: 'Lifecycle Stage',
    icon: Users,
    type: 'enum',
    options: [
      { value: 'new', label: 'New' },
      { value: 'onboarding', label: 'Onboarding' },
      { value: 'activated', label: 'Activated' },
      { value: 'engaged', label: 'Engaged' },
      { value: 'power_user', label: 'Power User' },
      { value: 'at_risk', label: 'At Risk' },
      { value: 'churning', label: 'Churning' },
      { value: 'churned', label: 'Churned' },
    ],
  },
  engagement_score: {
    label: 'Engagement Score',
    icon: Filter,
    type: 'number',
  },
  last_active: {
    label: 'Last Active',
    icon: Filter,
    type: 'date',
  },
  total_commands: {
    label: 'Total Commands',
    icon: Filter,
    type: 'number',
  },
  machine_count: {
    label: 'Machine Count',
    icon: Filter,
    type: 'number',
  },
  created_at: {
    label: 'Created At',
    icon: Filter,
    type: 'date',
  },
};

const OPERATORS: Record<SegmentOperator, { label: string; symbol: string }> = {
  equals: { label: 'equals', symbol: '=' },
  not_equals: { label: 'not equals', symbol: '≠' },
  greater_than: { label: 'greater than', symbol: '>' },
  less_than: { label: 'less than', symbol: '<' },
  contains: { label: 'contains', symbol: '∋' },
  between: { label: 'between', symbol: '↔' },
};

/** Get valid operators for a field type */
function getOperatorsForField(field: SegmentFieldType): SegmentOperator[] {
  const meta = FIELD_METADATA[field];
  switch (meta.type) {
    case 'enum':
      return ['equals', 'not_equals'];
    case 'number':
      return ['equals', 'not_equals', 'greater_than', 'less_than', 'between'];
    case 'date':
      return ['equals', 'greater_than', 'less_than', 'between'];
    case 'string':
      return ['equals', 'not_equals', 'contains'];
    default:
      return ['equals'];
  }
}

function generateId(): string {
  return Math.random().toString(36).substring(2, 11);
}

interface SegmentBuilderProps {
  /** Initial segment data */
  initialSegment?: Segment;
  /** Preview count of matching customers */
  previewCount?: number;
  /** Loading state for preview */
  previewLoading?: boolean;
  /** Callback when segment changes */
  onChange?: (segment: Segment) => void;
  /** Callback when segment is saved */
  onSave?: (segment: Segment) => void;
  /** Callback to request preview count update */
  onPreviewRequest?: (segment: Segment) => void;
  /** Custom class name */
  class?: string;
}

/** Dropdown select component */
const SegmentSelect: Component<{
  value: string;
  options: { value: string; label: string }[];
  onChange: (value: string) => void;
  placeholder?: string;
  class?: string;
}> = (props) => {
  const [isOpen, setIsOpen] = createSignal(false);
  
  const selectedLabel = createMemo(() => {
    const option = props.options.find(o => o.value === props.value);
    return option?.label || props.placeholder || 'Select...';
  });

  return (
    <div class={cn('relative', props.class)}>
      <button
        type="button"
        class={cn(
          'flex items-center justify-between gap-2 w-full px-3 py-2 rounded-xl',
          'bg-segment-builder-field border border-segment-builder-border',
          'text-sm font-mono text-nebula-200',
          'transition-all duration-300',
          'hover:border-segment-builder-border-hover',
          'focus:outline-none focus:ring-2 focus:ring-indigo-500/30',
          isOpen() && 'ring-2 ring-indigo-500/30'
        )}
        onClick={() => setIsOpen(!isOpen())}
        aria-expanded={isOpen()}
        aria-haspopup="listbox"
      >
        <span class={!props.value ? 'text-nebula-500' : ''}>
          {selectedLabel()}
        </span>
        <ChevronDown 
          size={14} 
          class={cn('text-nebula-500 transition-transform duration-300', isOpen() && 'rotate-180')} 
        />
      </button>
      
      <Show when={isOpen()}>
        <div 
          class="absolute z-50 w-full mt-1 py-1 rounded-xl bg-void-850 border border-white/10 shadow-xl"
          role="listbox"
        >
          <For each={props.options}>
            {(option) => (
              <button
                type="button"
                class={cn(
                  'w-full px-3 py-2 text-left text-sm font-mono',
                  'transition-colors duration-150',
                  option.value === props.value 
                    ? 'bg-indigo-500/20 text-indigo-300' 
                    : 'text-nebula-300 hover:bg-white/5'
                )}
                onClick={() => {
                  props.onChange(option.value);
                  setIsOpen(false);
                }}
                role="option"
                aria-selected={option.value === props.value}
              >
                {option.label}
              </button>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};

/** Input component for segment values */
const SegmentInput: Component<{
  type: 'text' | 'number' | 'date';
  value: string | number;
  onChange: (value: string | number) => void;
  placeholder?: string;
  class?: string;
}> = (props) => {
  return (
    <input
      type={props.type}
      value={String(props.value)}
      onInput={(e) => {
        const val = props.type === 'number' 
          ? parseFloat(e.currentTarget.value) || 0
          : e.currentTarget.value;
        props.onChange(val);
      }}
      placeholder={props.placeholder}
      class={cn(
        'px-3 py-2 rounded-xl w-full',
        'bg-segment-builder-value border border-segment-builder-border',
        'text-sm font-mono text-nebula-100 placeholder:text-nebula-600',
        'transition-all duration-300',
        'hover:border-segment-builder-border-hover',
        'focus:outline-none focus:ring-2 focus:ring-indigo-500/30 focus:border-transparent',
        props.class
      )}
    />
  );
};

/** Single condition row */
const ConditionRow: Component<{
  condition: SegmentCondition;
  onUpdate: (condition: SegmentCondition) => void;
  onRemove: () => void;
  isFirst: boolean;
  logic: LogicalOperator;
  onLogicChange: (logic: LogicalOperator) => void;
}> = (props) => {
  const fieldMeta = () => FIELD_METADATA[props.condition.field];
  const availableOperators = () => getOperatorsForField(props.condition.field);
  
  const inputType = () => {
    switch (fieldMeta().type) {
      case 'number': return 'number';
      case 'date': return 'date';
      default: return 'text';
    }
  };

  const isBetween = () => props.condition.operator === 'between';

  return (
    <div class="flex items-center gap-2 group">
      <div class="cursor-grab opacity-0 group-hover:opacity-50 transition-opacity duration-300">
        <GripVertical size={14} class="text-nebula-600" />
      </div>

      <Show when={!props.isFirst}>
        <button
          type="button"
          class={cn(
            'px-2 py-1 rounded-lg text-2xs font-bold uppercase tracking-widest',
            'transition-all duration-300',
            props.logic === 'AND' 
              ? 'bg-indigo-500/20 text-indigo-300' 
              : 'bg-photon-500/20 text-photon-300'
          )}
          onClick={() => props.onLogicChange(props.logic === 'AND' ? 'OR' : 'AND')}
          aria-label={`Change logic to ${props.logic === 'AND' ? 'OR' : 'AND'}`}
        >
          {props.logic}
        </button>
      </Show>
      <Show when={props.isFirst}>
        <div class="w-10" />
      </Show>

      <SegmentSelect
        value={props.condition.field}
        options={Object.entries(FIELD_METADATA).map(([key, meta]) => ({
          value: key,
          label: meta.label,
        }))}
        onChange={(value) => props.onUpdate({ 
          ...props.condition, 
          field: value as SegmentFieldType,
          operator: getOperatorsForField(value as SegmentFieldType)[0],
          value: '',
        })}
        class="w-40"
      />

      <SegmentSelect
        value={props.condition.operator}
        options={availableOperators().map((op) => ({
          value: op,
          label: `${OPERATORS[op].symbol} ${OPERATORS[op].label}`,
        }))}
        onChange={(value) => props.onUpdate({ 
          ...props.condition, 
          operator: value as SegmentOperator,
          value: value === 'between' ? [0, 100] : '',
        })}
        class="w-36"
      />

      <Show when={fieldMeta().options && !isBetween()}>
        <SegmentSelect
          value={String(props.condition.value)}
          options={fieldMeta().options!}
          onChange={(value) => props.onUpdate({ ...props.condition, value })}
          placeholder="Select value..."
          class="flex-1 min-w-[140px]"
        />
      </Show>

      <Show when={!fieldMeta().options && !isBetween()}>
        <SegmentInput
          type={inputType()}
          value={props.condition.value as string | number}
          onChange={(value) => props.onUpdate({ ...props.condition, value })}
          placeholder={inputType() === 'date' ? 'YYYY-MM-DD' : 'Enter value...'}
          class="flex-1 min-w-[140px]"
        />
      </Show>

      <Show when={isBetween()}>
        <div class="flex items-center gap-2 flex-1 min-w-[200px]">
          <SegmentInput
            type={inputType()}
            value={(props.condition.value as [number, number])[0]}
            onChange={(value) => {
              const current = props.condition.value as [number, number];
              props.onUpdate({ ...props.condition, value: [value as number, current[1]] });
            }}
            placeholder="Min"
            class="w-24"
          />
          <span class="text-nebula-500 text-xs font-mono">to</span>
          <SegmentInput
            type={inputType()}
            value={(props.condition.value as [number, number])[1]}
            onChange={(value) => {
              const current = props.condition.value as [number, number];
              props.onUpdate({ ...props.condition, value: [current[0], value as number] });
            }}
            placeholder="Max"
            class="w-24"
          />
        </div>
      </Show>

      <button
        type="button"
        class={cn(
          'p-2 rounded-lg opacity-0 group-hover:opacity-100',
          'text-flare-400 hover:bg-flare-500/10',
          'transition-all duration-300'
        )}
        onClick={props.onRemove}
        aria-label="Remove condition"
      >
        <Trash2 size={14} />
      </button>
    </div>
  );
};

const ConditionGroup: Component<{
  group: SegmentGroup;
  onUpdate: (group: SegmentGroup) => void;
  onRemove: () => void;
  isFirst: boolean;
  rootLogic: LogicalOperator;
  onRootLogicChange: (logic: LogicalOperator) => void;
}> = (props) => {
  const addCondition = () => {
    const newCondition: SegmentCondition = {
      id: generateId(),
      field: 'tier',
      operator: 'equals',
      value: '',
    };
    props.onUpdate({
      ...props.group,
      conditions: [...props.group.conditions, newCondition],
    });
  };

  const updateCondition = (index: number, condition: SegmentCondition) => {
    const newConditions = [...props.group.conditions];
    newConditions[index] = condition;
    props.onUpdate({ ...props.group, conditions: newConditions });
  };

  const removeCondition = (index: number) => {
    if (props.group.conditions.length === 1) {
      props.onRemove();
    } else {
      const newConditions = props.group.conditions.filter((_, i) => i !== index);
      props.onUpdate({ ...props.group, conditions: newConditions });
    }
  };

  return (
    <div class="relative">
      <Show when={!props.isFirst}>
        <div class="flex items-center gap-3 mb-3">
          <div class="h-px flex-1 bg-gradient-to-r from-transparent via-white/10 to-transparent" />
          <button
            type="button"
            class={cn(
              'px-3 py-1 rounded-full text-xs font-bold uppercase tracking-widest',
              'transition-all duration-300',
              props.rootLogic === 'AND' 
                ? 'bg-indigo-500/20 text-indigo-300 hover:bg-indigo-500/30' 
                : 'bg-photon-500/20 text-photon-300 hover:bg-photon-500/30'
            )}
            onClick={() => props.onRootLogicChange(props.rootLogic === 'AND' ? 'OR' : 'AND')}
          >
            {props.rootLogic}
          </button>
          <div class="h-px flex-1 bg-gradient-to-r from-transparent via-white/10 to-transparent" />
        </div>
      </Show>

      <div 
        class={cn(
          'relative p-4 rounded-2xl',
          'bg-void-900/50 border border-white/5',
          'transition-all duration-300',
          'hover:border-white/10'
        )}
      >
        <div class="absolute top-2 right-2 flex items-center gap-1">
          <button
            type="button"
            class={cn(
              'p-1.5 rounded-lg text-nebula-500',
              'hover:text-flare-400 hover:bg-flare-500/10',
              'transition-all duration-300'
            )}
            onClick={props.onRemove}
            aria-label="Remove group"
          >
            <X size={14} />
          </button>
        </div>

        <div class="space-y-3">
          <For each={props.group.conditions}>
            {(condition, index) => (
              <ConditionRow
                condition={condition}
                onUpdate={(c) => updateCondition(index(), c)}
                onRemove={() => removeCondition(index())}
                isFirst={index() === 0}
                logic={props.group.logic}
                onLogicChange={(logic) => props.onUpdate({ ...props.group, logic })}
              />
            )}
          </For>
        </div>

        <button
          type="button"
          class={cn(
            'mt-4 flex items-center gap-2 px-3 py-2 rounded-xl w-full',
            'text-sm font-medium text-nebula-400',
            'border border-dashed border-white/10',
            'transition-all duration-300',
            'hover:border-indigo-500/30 hover:text-indigo-300 hover:bg-indigo-500/5'
          )}
          onClick={addCondition}
        >
          <Plus size={14} />
          <span>Add condition</span>
        </button>
      </div>
    </div>
  );
};

const SaveSegmentModal: Component<{
  isOpen: boolean;
  name: string;
  description: string;
  onNameChange: (name: string) => void;
  onDescriptionChange: (description: string) => void;
  onSave: () => void;
  onClose: () => void;
}> = (props) => {
  return (
    <Show when={props.isOpen}>
      <div class="fixed inset-0 z-50 flex items-center justify-center">
        <div 
          class="absolute inset-0 bg-void-950/80 backdrop-blur-sm"
          onClick={props.onClose}
        />
        
        <div 
          class={cn(
            'relative w-full max-w-md p-6 rounded-3xl',
            'bg-void-850 border border-white/10',
            'shadow-2xl'
          )}
          role="dialog"
          aria-modal="true"
          aria-labelledby="save-segment-title"
        >
          <h3 
            id="save-segment-title"
            class="text-lg font-display font-bold text-white mb-4"
          >
            Save Segment
          </h3>

          <div class="space-y-4">
            <div>
              <label class="block text-xs font-bold uppercase tracking-widest text-nebula-500 mb-2">
                Segment Name
              </label>
              <input
                type="text"
                value={props.name}
                onInput={(e) => props.onNameChange(e.currentTarget.value)}
                placeholder="e.g., High-Value At-Risk"
                class={cn(
                  'w-full px-4 py-3 rounded-xl',
                  'bg-void-900 border border-white/10',
                  'text-white placeholder:text-nebula-600',
                  'transition-all duration-300',
                  'focus:outline-none focus:ring-2 focus:ring-indigo-500/30'
                )}
              />
            </div>

            <div>
              <label class="block text-xs font-bold uppercase tracking-widest text-nebula-500 mb-2">
                Description
              </label>
              <textarea
                value={props.description}
                onInput={(e) => props.onDescriptionChange(e.currentTarget.value)}
                placeholder="Describe what this segment represents..."
                rows={3}
                class={cn(
                  'w-full px-4 py-3 rounded-xl resize-none',
                  'bg-void-900 border border-white/10',
                  'text-white placeholder:text-nebula-600',
                  'transition-all duration-300',
                  'focus:outline-none focus:ring-2 focus:ring-indigo-500/30'
                )}
              />
            </div>
          </div>

          <div class="flex items-center justify-end gap-3 mt-6">
            <button
              type="button"
              class={cn(
                'px-4 py-2 rounded-xl',
                'text-sm font-medium text-nebula-400',
                'transition-all duration-300',
                'hover:bg-white/5 hover:text-white'
              )}
              onClick={props.onClose}
            >
              Cancel
            </button>
            <button
              type="button"
              class={cn(
                'px-4 py-2 rounded-xl',
                'bg-indigo-500 text-white text-sm font-medium',
                'transition-all duration-300',
                'hover:bg-indigo-400',
                'disabled:opacity-50 disabled:cursor-not-allowed'
              )}
              disabled={!props.name.trim()}
              onClick={props.onSave}
            >
              Save Segment
            </button>
          </div>
        </div>
      </div>
    </Show>
  );
};

export const SegmentBuilder: Component<SegmentBuilderProps> = (props) => {
  const defaultSegment: Segment = {
    id: generateId(),
    name: '',
    description: '',
    groups: [
      {
        id: generateId(),
        logic: 'AND',
        conditions: [
          {
            id: generateId(),
            field: 'tier',
            operator: 'equals',
            value: '',
          },
        ],
      },
    ],
    rootLogic: 'AND',
  };

  const [segment, setSegment] = createSignal<Segment>(props.initialSegment || defaultSegment);
  const [showSaveModal, setShowSaveModal] = createSignal(false);

  createEffect(() => {
    const currentSegment = segment();
    props.onChange?.(currentSegment);
    props.onPreviewRequest?.(currentSegment);
  });

  const addGroup = () => {
    const newGroup: SegmentGroup = {
      id: generateId(),
      logic: 'AND',
      conditions: [
        {
          id: generateId(),
          field: 'tier',
          operator: 'equals',
          value: '',
        },
      ],
    };
    setSegment((s) => ({ ...s, groups: [...s.groups, newGroup] }));
  };

  const updateGroup = (index: number, group: SegmentGroup) => {
    setSegment((s) => {
      const newGroups = [...s.groups];
      newGroups[index] = group;
      return { ...s, groups: newGroups };
    });
  };

  const removeGroup = (index: number) => {
    setSegment((s) => {
      if (s.groups.length === 1) {
        return defaultSegment;
      }
      return { ...s, groups: s.groups.filter((_, i) => i !== index) };
    });
  };

  const handleSave = () => {
    props.onSave?.(segment());
    setShowSaveModal(false);
  };

  const conditionCount = createMemo(() => 
    segment().groups.reduce((sum, g) => sum + g.conditions.length, 0)
  );

  return (
    <div class={cn('space-y-6', props.class)}>
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <div class="p-2.5 rounded-xl bg-indigo-500/10">
            <Filter size={20} class="text-indigo-400" />
          </div>
          <div>
            <h3 class="text-lg font-display font-bold text-white">
              Segment Builder
            </h3>
            <p class="text-sm text-nebula-500">
              {conditionCount()} condition{conditionCount() !== 1 ? 's' : ''} in {segment().groups.length} group{segment().groups.length !== 1 ? 's' : ''}
            </p>
          </div>
        </div>

        <div class="flex items-center gap-4">
          <div 
            class={cn(
              'flex items-center gap-2 px-4 py-2 rounded-xl',
              'bg-void-800 border border-white/5'
            )}
            role="status"
            aria-live="polite"
          >
            <Users size={16} class="text-electric-400" />
            <Show 
              when={!props.previewLoading} 
              fallback={
                <div class="h-5 w-16 rounded bg-white/5 animate-pulse" />
              }
            >
              <span class="font-mono font-bold text-white tabular-nums">
                {props.previewCount?.toLocaleString() ?? '—'}
              </span>
            </Show>
            <span class="text-sm text-nebula-500">matching</span>
          </div>

          <button
            type="button"
            class={cn(
              'flex items-center gap-2 px-4 py-2 rounded-xl',
              'bg-indigo-500 text-white font-medium',
              'transition-all duration-300',
              'hover:bg-indigo-400 hover:shadow-glow-sm'
            )}
            onClick={() => setShowSaveModal(true)}
          >
            <Save size={16} />
            <span>Save</span>
          </button>
        </div>
      </div>

      <div class="space-y-4">
        <For each={segment().groups}>
          {(group, index) => (
            <ConditionGroup
              group={group}
              onUpdate={(g) => updateGroup(index(), g)}
              onRemove={() => removeGroup(index())}
              isFirst={index() === 0}
              rootLogic={segment().rootLogic}
              onRootLogicChange={(logic) => setSegment((s) => ({ ...s, rootLogic: logic }))}
            />
          )}
        </For>
      </div>

      <button
        type="button"
        class={cn(
          'flex items-center justify-center gap-2 w-full px-4 py-4 rounded-2xl',
          'text-sm font-medium text-nebula-400',
          'border-2 border-dashed border-white/10',
          'transition-all duration-300',
          'hover:border-indigo-500/30 hover:text-indigo-300 hover:bg-indigo-500/5'
        )}
        onClick={addGroup}
      >
        <Layers size={16} />
        <span>Add condition group</span>
      </button>

      <SaveSegmentModal
        isOpen={showSaveModal()}
        name={segment().name}
        description={segment().description}
        onNameChange={(name) => setSegment((s) => ({ ...s, name }))}
        onDescriptionChange={(description) => setSegment((s) => ({ ...s, description }))}
        onSave={handleSave}
        onClose={() => setShowSaveModal(false)}
      />
    </div>
  );
};

export default SegmentBuilder;
